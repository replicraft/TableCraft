//! Independent catalog forwarding, descriptor and single-page LVAL framing.
use super::super::{DIRECTORY, PAGE, word};
use crate::fixtures::access::{put16, put32};

const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const MAP_PAGE: usize = 3;
const ROOT_SLOT: usize = 7;
const FORWARD_SLOT: usize = ROOT_SLOT + 1;
const ROW_COUNT_OFFSET: usize = 12;
const FREE_OFFSET: usize = 2;
const ROW_MASK_AND_COUNT_BYTES: usize = 5;
const PROPERTY_VARIABLE: usize = 8;
const VARIABLE_COLUMNS: usize = 12;
const FORWARD_FLAG: usize = 0x4000;
const TARGET_FLAG: usize = 0x8000;
const OFFSET_MASK: usize = 0x1fff;
const SHORT_BYTES: usize = size_of::<u16>();
const LONG_BYTES: usize = size_of::<u32>();
const INLINE_FLAG: u32 = 0x8000_0000;
const SINGLE_FLAG: u32 = 0x4000_0000;
const LENGTH_MASK: u32 = 0x3fff_ffff;
const DESCRIPTOR_BYTES: usize = LONG_BYTES * 3;
const REFERENCE_SHIFT: u32 = 8;
const REFERENCE_SLOT_MASK: u32 = 255;
const MAP_SLOTS: [usize; 2] = [16, 17];
const MAP_ROW_BYTES: usize = 13;
const MAP_BITS_OFFSET: usize = 6;
const PROPERTY_OWNED_BIT: u8 = 4;
const PROPERTY_PRESENT_MASK: u8 = 0x40;
const DATA_PAGE_PREFIX: [u8; 2] = [1, 1];
const LVAL_SIGNATURE_OFFSET: usize = 4;

struct Record<'a> {
    page: &'a [u8],
    row: &'a [u8],
    slot: usize,
    start: usize,
    end: usize,
    begin: usize,
    finish: usize,
}

fn record(before: &[u8]) -> Record<'_> {
    let page = &before[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE];
    let root = word(page, DIRECTORY + ROOT_SLOT * SHORT_BYTES);
    let slot = if root & FORWARD_FLAG != 0 {
        let start = root & OFFSET_MASK;
        let reference = u32::from_le_bytes(page[start..start + LONG_BYTES].try_into().unwrap());
        assert_eq!(reference >> REFERENCE_SHIFT, CATALOG_PAGE as u32);
        (reference & REFERENCE_SLOT_MASK) as usize
    } else {
        ROOT_SLOT
    };
    let start = word(page, DIRECTORY + slot * SHORT_BYTES) & OFFSET_MASK;
    let end = word(page, DIRECTORY + (slot - 1) * SHORT_BYTES) & OFFSET_MASK;
    let row = &page[start..end];
    let footer = row.len() - ROW_MASK_AND_COUNT_BYTES;
    let begin = word(row, footer - SHORT_BYTES * (PROPERTY_VARIABLE + 1));
    let finish = word(row, footer - SHORT_BYTES * (PROPERTY_VARIABLE + 2));
    Record {
        page,
        row,
        slot,
        start,
        end,
        begin,
        finish,
    }
}

pub(super) fn body(before: &[u8]) -> Vec<u8> {
    read_body(before, |page, slot| {
        assert_eq!(slot, word(page, ROW_COUNT_OFFSET) - 1);
    })
}

/// Select the user body through its actual descriptor after a separate database
/// properties ROW has appended. This deliberately does not relax body()'s
/// latest-row-only fixture profile.
pub(super) fn selected_body(before: &[u8]) -> Vec<u8> {
    let record = record(before);
    let descriptor = &record.row[record.begin..record.finish];
    assert_eq!(descriptor.len(), DESCRIPTOR_BYTES);
    assert_eq!(
        u32::from_le_bytes(descriptor[..LONG_BYTES].try_into().unwrap()) & !LENGTH_MASK,
        SINGLE_FLAG
    );
    assert_eq!(&descriptor[LONG_BYTES * 2..], &[0; LONG_BYTES]);
    read_body(before, |page, slot| {
        assert!(slot < word(page, ROW_COUNT_OFFSET));
        assert_eq!(word(page, DIRECTORY + slot * SHORT_BYTES) & !OFFSET_MASK, 0);
        if slot > 0 {
            assert_eq!(
                word(page, DIRECTORY + (slot - 1) * SHORT_BYTES) & !OFFSET_MASK,
                0
            );
        }
    })
}

/// Current nullable user properties may be genuinely absent before the first
/// field declaration. Existing selected_body retains its Single-only contract.
pub(super) fn current_body(before: &[u8]) -> Vec<u8> {
    let record = record(before);
    if record.begin == record.finish {
        Vec::new()
    } else {
        selected_body(before)
    }
}

fn read_body(before: &[u8], check_slot: impl FnOnce(&[u8], usize)) -> Vec<u8> {
    let record = record(before);
    if record.begin == record.finish {
        return Vec::new();
    }
    let descriptor = &record.row[record.begin..record.finish];
    let tagged = u32::from_le_bytes(descriptor[..LONG_BYTES].try_into().unwrap());
    let length = (tagged & LENGTH_MASK) as usize;
    if tagged & !LENGTH_MASK == INLINE_FLAG {
        assert_eq!(descriptor.len(), DESCRIPTOR_BYTES + length);
        return descriptor[DESCRIPTOR_BYTES..].to_vec();
    }
    assert_eq!(tagged & !LENGTH_MASK, SINGLE_FLAG);
    let reference = u32::from_le_bytes(descriptor[LONG_BYTES..LONG_BYTES * 2].try_into().unwrap());
    assert_eq!(reference >> REFERENCE_SHIFT, PROPERTY_PAGE as u32);
    let page = &before[PROPERTY_PAGE * PAGE..(PROPERTY_PAGE + 1) * PAGE];
    let slot = (reference & REFERENCE_SLOT_MASK) as usize;
    check_slot(page, slot);
    let start = word(page, DIRECTORY + slot * SHORT_BYTES);
    let end = if slot == 0 {
        PAGE
    } else {
        word(page, DIRECTORY + (slot - 1) * SHORT_BYTES)
    };
    assert_eq!(end - start, length);
    page[start..end].to_vec()
}

pub(super) fn append(before: &[u8], mut after: Vec<u8>, body: &[u8]) -> Vec<u8> {
    let reference = append_body(before, &mut after, body);
    let catalog = parent_page(before, body.len(), reference);
    after[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE].copy_from_slice(&catalog);
    after
}

/// The first user property descriptor can follow the separately stored database
/// role. Its compact catalog growth retains that role and both physical slots.
pub(in crate::fixtures::access) fn append_current(
    before: &[u8],
    mut after: Vec<u8>,
    body: &[u8],
) -> Vec<u8> {
    let original = record(before);
    if original.begin != original.finish {
        return append(before, after, body);
    }
    let reference = append_body(before, &mut after, body);
    let catalog = parent_page_with_frontier(before, body.len(), reference, true);
    after[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE].copy_from_slice(&catalog);
    after
}

fn append_body(before: &[u8], after: &mut Vec<u8>, body: &[u8]) -> u32 {
    let (mut page, count, end) = if before.len() == PROPERTY_PAGE * PAGE {
        let mut page = vec![0; PAGE];
        page[..2].copy_from_slice(&DATA_PAGE_PREFIX);
        page[LVAL_SIGNATURE_OFFSET..LVAL_SIGNATURE_OFFSET + LONG_BYTES].copy_from_slice(b"LVAL");
        for slot in MAP_SLOTS {
            let row =
                MAP_PAGE * PAGE + word(after, MAP_PAGE * PAGE + DIRECTORY + SHORT_BYTES * slot);
            assert_eq!(&after[row..row + MAP_ROW_BYTES], &[0; MAP_ROW_BYTES]);
            after[row + MAP_BITS_OFFSET] |= PROPERTY_OWNED_BIT;
        }
        after.extend(&page);
        (page, 0, PAGE)
    } else {
        assert_eq!(before.len(), (PROPERTY_PAGE + 1) * PAGE);
        let page = before[PROPERTY_PAGE * PAGE..(PROPERTY_PAGE + 1) * PAGE].to_vec();
        let count = word(&page, ROW_COUNT_OFFSET);
        let end = word(&page, DIRECTORY + SHORT_BYTES * (count - 1));
        (page, count, end)
    };
    let start = end.checked_sub(body.len()).unwrap();
    let directory_end = DIRECTORY + SHORT_BYTES * (count + 1);
    assert!(start >= directory_end);
    assert!(page[start..end].iter().all(|byte| *byte == 0));
    page[start..end].copy_from_slice(body);
    put16(&mut page, DIRECTORY + SHORT_BYTES * count, start as u16);
    put16(&mut page, ROW_COUNT_OFFSET, (count + 1) as u16);
    put16(&mut page, FREE_OFFSET, (start - directory_end) as u16);
    after[PROPERTY_PAGE * PAGE..(PROPERTY_PAGE + 1) * PAGE].copy_from_slice(&page);
    (PROPERTY_PAGE as u32) << REFERENCE_SHIFT | count as u32
}

fn parent_page(before: &[u8], body_length: usize, reference: u32) -> Vec<u8> {
    parent_page_with_frontier(before, body_length, reference, false)
}

fn parent_page_with_frontier(
    before: &[u8],
    body_length: usize,
    reference: u32,
    current: bool,
) -> Vec<u8> {
    let record = record(before);
    let mut descriptor = (SINGLE_FLAG | body_length as u32).to_le_bytes().to_vec();
    descriptor.extend(reference.to_le_bytes());
    descriptor.extend([0; LONG_BYTES]);
    let length = record
        .row
        .len()
        .max(record.row.len() - (record.finish - record.begin) + descriptor.len());
    let mut row = vec![0; length];
    row[..record.begin].copy_from_slice(&record.row[..record.begin]);
    row[record.begin..record.begin + descriptor.len()].copy_from_slice(&descriptor);
    let footer = &record.row[record.finish..];
    row[length - footer.len()..].copy_from_slice(footer);
    let count = length - ROW_MASK_AND_COUNT_BYTES;
    for index in PROPERTY_VARIABLE + 1..VARIABLE_COLUMNS {
        put16(
            &mut row,
            count - SHORT_BYTES * (index + 1),
            (record.begin + descriptor.len()) as u16,
        );
    }
    row[length - 2] |= PROPERTY_PRESENT_MASK;
    let mut page = record.page.to_vec();
    if row.len() == record.row.len() {
        page[record.start..record.end].copy_from_slice(&row);
    } else if current {
        assert_eq!(record.slot, ROOT_SLOT);
        let count = word(&page, ROW_COUNT_OFFSET);
        assert_eq!(count, 9);
        assert_eq!(record.begin, record.finish);
        let growth = row.len() - record.row.len();
        let frontier = word(&page, DIRECTORY + (count - 1) * SHORT_BYTES) & OFFSET_MASK;
        let new_frontier = frontier.checked_sub(growth).unwrap();
        assert!(new_frontier >= DIRECTORY + count * SHORT_BYTES);
        assert!(page[new_frontier..frontier].iter().all(|byte| *byte == 0));
        page.copy_within(frontier..record.start, new_frontier);
        page[record.start - growth..record.end].copy_from_slice(&row);
        for slot in record.slot..count {
            let old = word(record.page, DIRECTORY + slot * SHORT_BYTES);
            assert_eq!(old & !OFFSET_MASK, 0);
            put16(
                &mut page,
                DIRECTORY + slot * SHORT_BYTES,
                (old - growth) as u16,
            );
        }
        put16(
            &mut page,
            FREE_OFFSET,
            (new_frontier - DIRECTORY - count * SHORT_BYTES) as u16,
        );
    } else {
        assert_eq!(record.slot, ROOT_SLOT);
        let target_slot = word(&page, ROW_COUNT_OFFSET);
        assert_eq!(target_slot, FORWARD_SLOT);
        let frontier = word(&page, DIRECTORY + (target_slot - 1) * SHORT_BYTES) & OFFSET_MASK;
        let target = frontier.checked_sub(row.len()).unwrap();
        assert!(target >= DIRECTORY + (target_slot + 1) * SHORT_BYTES);
        assert!(page[target..frontier].iter().all(|byte| *byte == 0));
        page[target..frontier].copy_from_slice(&row);
        page[record.start..record.end].fill(0);
        put32(
            &mut page,
            record.start,
            (CATALOG_PAGE as u32) << REFERENCE_SHIFT | target_slot as u32,
        );
        put16(
            &mut page,
            DIRECTORY + ROOT_SLOT * SHORT_BYTES,
            (record.start | FORWARD_FLAG) as u16,
        );
        put16(
            &mut page,
            DIRECTORY + target_slot * SHORT_BYTES,
            (target | TARGET_FLAG) as u16,
        );
        put16(&mut page, ROW_COUNT_OFFSET, (target_slot + 1) as u16);
        put16(
            &mut page,
            FREE_OFFSET,
            (target - (DIRECTORY + (target_slot + 1) * SHORT_BYTES)) as u16,
        );
    }
    page
}
