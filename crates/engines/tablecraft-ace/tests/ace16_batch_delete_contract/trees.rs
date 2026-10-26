//! Independent two-owner trees whose retirement bits share one global map byte.
use super::{model, rows, support};
use model::{PAGE_BYTES, budget, capture, requests, table};
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, TableRowDeletes, check_table, check_table_constraints};
use tablecraft_model::CatalogScope;

const ENTRY_START: usize = 480;
const BITMAP: usize = 27;
const HEIGHT: usize = 26;
const PREVIOUS: usize = 12;
const NEXT: usize = 16;
const OWNER: usize = 4;
const FREE_BYTES: usize = 2;
const SLOT_START: usize = 14;
const MAP_HEADER: usize = 5;
const ROW_COUNT: usize = 16;
const CHILDREN: [[usize; 2]; 2] = [[25, 26], [27, 28]];

fn map_start(bytes: &[u8], page: usize) -> usize {
    let at = page * PAGE_BYTES + SLOT_START;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    page * PAGE_BYTES + usize::from(word & 0x1fff) + MAP_HEADER
}

fn entries(page: &[u8]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut start = 0;

    for end in 1..=PAGE_BYTES - ENTRY_START {
        if page[BITMAP + end / 8] & (1 << (end % 8)) != 0 {
            result.push(page[ENTRY_START + start..ENTRY_START + end].to_vec());
            start = end;
        }
    }

    result
}

fn packet(owner: usize, height: u8, previous: usize, next: usize, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0; PAGE_BYTES];
    page[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut page, OWNER, owner as u32);
    support::put32(&mut page, PREVIOUS, previous as u32);
    support::put32(&mut page, NEXT, next as u32);
    page[HEIGHT] = height;

    let mut end = 0;
    for entry in entries {
        page[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        page[BITMAP + end / 8] |= 1 << (end % 8);
    }

    support::put16(
        &mut page,
        FREE_BYTES,
        (PAGE_BYTES - ENTRY_START - end) as u16,
    );
    page
}

fn database(code: u8) -> model::Model {
    let mut model = model::database(code, false, false, false);
    model.bytes.resize(29 * PAGE_BYTES, 0);
    let global = map_start(&model.bytes, 1);

    for (target, children) in CHILDREN.into_iter().enumerate() {
        let root = model::root(target);
        let original = &model.bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES];
        let entries = entries(original);
        let split = entries.len().div_ceil(2);
        let leaves = [&entries[..split], &entries[split..]];
        let mut bounds = Vec::new();

        for (position, &number) in children.iter().enumerate() {
            let previous = if position == 0 { 0 } else { children[0] };
            let next = if position == 0 { children[1] } else { 0 };
            let page = packet(
                model::definition(target),
                0,
                previous,
                next,
                leaves[position],
            );
            model.bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(&page);

            let mut maximum = leaves[position].last().unwrap().clone();
            maximum.extend((number as u32).to_be_bytes());
            bounds.push(maximum);

            let owned = map_start(&model.bytes, model::definition(target) + 3);
            model.bytes[owned + number / 8] |= 1 << (number % 8);
            model.bytes[global + number / 8] &= !(1 << (number % 8));
        }

        let root_page = packet(model::definition(target), 1, 0, 0, &bounds);
        model.bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES].copy_from_slice(&root_page);
    }

    model
}

fn checked(bytes: &[u8]) {
    for target in 0..3 {
        let report = check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            report
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
fn two_table_contraction_combines_shared_global_byte_and_preserves_each_owned_map() {
    for code in [5, 6] {
        let model = database(code);
        checked(&model.bytes);
        let child_rows = requests(&model, 0, &[0, 1, 2, 3]);
        let parent_rows = requests(&model, 1, &[0, 1, 2]);
        let global = map_start(&model.bytes, 1);
        assert_eq!(model.bytes[global + 3], 0xe0);

        let mut results = Vec::new();
        for reverse in [false, true] {
            let mut groups = [
                TableRowDeletes::new(table(0), &child_rows),
                TableRowDeletes::new(table(1), &parent_rows),
            ];
            if reverse {
                groups.reverse();
            }

            let candidate = capture(&model.bytes)
                .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
                .unwrap();
            let after = candidate.candidate_bytes();
            checked(after);
            assert_eq!(candidate.deleted_rows(), 7);
            assert_eq!(after.len(), model.bytes.len());
            assert_eq!(after[global + 3], 0xfe);
            let mut allowed = vec![false; after.len()];
            allowed[global + 3] = true;

            for (target, children) in CHILDREN.into_iter().enumerate() {
                assert!(rows(after, table(target)).is_empty());
                let owned = map_start(&model.bytes, model::definition(target) + 3);
                let root = model::root(target);
                allowed[owned + 3] = true;
                allowed[root * PAGE_BYTES..(root + 1) * PAGE_BYTES].fill(true);
                let count = model::definition(target) * PAGE_BYTES + ROW_COUNT;
                allowed[count..count + 4].fill(true);
                assert_eq!(after[root * PAGE_BYTES], 4);
                assert_eq!(after[root * PAGE_BYTES + HEIGHT], 0);
                assert_ne!(after[owned + root / 8] & (1 << (root % 8)), 0);

                for number in children {
                    allowed[number * PAGE_BYTES..number * PAGE_BYTES + 4].fill(true);
                    assert_eq!(after[owned + number / 8] & (1 << (number % 8)), 0);
                    assert_eq!(
                        &after[number * PAGE_BYTES..number * PAGE_BYTES + 4],
                        &[0; 4]
                    );
                    assert_eq!(
                        &after[number * PAGE_BYTES + 4..(number + 1) * PAGE_BYTES],
                        &model.bytes[number * PAGE_BYTES + 4..(number + 1) * PAGE_BYTES]
                    );
                }

                for (ordinal, record) in model.records[target].iter().enumerate() {
                    let slot = model::data(target) * PAGE_BYTES + SLOT_START + ordinal * 2;
                    let word = u16::from_le_bytes(model.bytes[slot..slot + 2].try_into().unwrap());
                    assert_eq!(&after[slot..slot + 2], &(word | 0xc000).to_le_bytes());
                    allowed[slot..slot + 2].fill(true);

                    let start = model::data(target) * PAGE_BYTES + usize::from(word & 0x1fff);
                    assert_eq!(&after[start..start + record.len()], record);
                }
            }

            for (offset, (&before, &actual)) in model.bytes.iter().zip(after).enumerate() {
                if !allowed[offset] {
                    assert_eq!(actual, before, "unexpected mutation at {offset}");
                }
            }

            let untouched = model::definition(2) * PAGE_BYTES;
            assert_eq!(
                &after[untouched..25 * PAGE_BYTES],
                &model.bytes[untouched..25 * PAGE_BYTES]
            );
            results.push(after.to_vec());
        }

        assert_eq!(results[0], results[1]);
    }
}

#[test]
fn batch_map_composition_does_not_authorize_free_or_cross_owned_original_pages() {
    let original = database(6);
    let children = requests(&original, 0, &[0, 1, 2, 3]);
    let parents = requests(&original, 1, &[0, 1, 2]);
    let groups = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &parents),
    ];

    let mut already_free = original.bytes.clone();
    let global = map_start(&already_free, 1);
    already_free[global + 25 / 8] |= 1 << (25 % 8);

    let mut cross_owned = original.bytes.clone();
    let parent_owned = map_start(&cross_owned, model::definition(1) + 3);
    cross_owned[parent_owned + 25 / 8] |= 1 << (25 % 8);

    for bytes in [already_free, cross_owned] {
        let retained = bytes.clone();
        assert!(
            capture(&bytes)
                .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(bytes, retained);
    }

    let candidate = capture(&original.bytes)
        .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.before_bytes(), original.bytes);
}
