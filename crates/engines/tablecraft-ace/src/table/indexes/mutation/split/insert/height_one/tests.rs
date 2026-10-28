use super::*;
use crate::{
    IndexCheckStatus, check_table, check_table_constraints,
    format::allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
    table::{indexes::page::KeyRef, mutation::append},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;

mod model;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn checked(bytes: &[u8], aliases: bool) -> crate::TableCheck {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 452);
    assert_eq!(checked.indexes().len(), if aliases { 2 } else { 1 });
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 452);
    checked
}

fn operation<'a>(
    image: &'a NativeImage,
    checked: &'a crate::TableCheck,
    keys: &'a [KeyRef],
) -> Operation<'a> {
    Operation {
        image,
        checked,
        id: 0,
        final_keys: keys,
    }
}

fn seeded(image: &NativeImage) -> State {
    let mut append = State::new(image, 10);
    let number = image.as_bytes().len() as u32 / 4096;
    let mut source = image.reader();
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    append
        .reserve_data(number, reader.budget, reader.cancelled)
        .unwrap();
    for (reference, role, set) in [
        (11 << 8, MapRole::Owned, true),
        ((11 << 8) | 1, MapRole::TableFreeSpace, true),
        (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree, false),
    ] {
        let map = RepresentedMap::read(&mut reader, reference, role).unwrap();
        let cell = map.future_cell(&mut reader, number).unwrap();
        append::claim(&mut append.map_edits, cell, set, &mut reader).unwrap();
    }
    append
}

fn compose(before: &[u8], plan: &Plan) -> Vec<u8> {
    let mut bytes = before.to_vec();
    bytes.resize(plan.append.length, 0);
    for patch in &plan.patches {
        assert_eq!(
            &bytes[patch.offset..patch.offset + patch.before.len()],
            patch.before
        );
        bytes[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    for promotion in &plan.promotions {
        assert_eq!(
            &bytes[promotion.offset..promotion.offset + promotion.before.len()],
            promotion.before
        );
        bytes[promotion.offset..promotion.offset + promotion.after.len()]
            .copy_from_slice(&promotion.after);
    }
    for edit in &plan.append.map_edits {
        assert_eq!(bytes[edit.offset], edit.before);
        bytes[edit.offset] = edit.after;
    }
    for page in &plan.added {
        let start = page.number as usize * 4096;
        bytes[start..start + 4096].copy_from_slice(&page.bytes);
    }
    bytes
}

#[test]
fn left_or_right_child_splits_once_with_fitting_peer_bulk_and_aliases() {
    for code in [5, 6] {
        for aliases in [false, true] {
            for right in [false, true] {
                for peer in [false, true] {
                    let source = model::source(code, aliases, right, false);
                    let check = checked(&source.bytes, aliases);
                    let image = capture(&source.bytes);
                    let keys = model::final_keys(&source, false, peer);
                    let expected = model::expected(&source, &keys, false);
                    let plan = continue_primary(
                        operation(&image, &check, &keys),
                        State::new(&image, 10),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(plan.append.original_length, source.bytes.len());
                    assert_eq!(plan.append.length, source.bytes.len() + 4096);
                    assert_eq!(plan.append.indexes().count(), 1);
                    assert!(plan.promotions.is_empty());
                    assert_eq!(plan.added.len(), 1);
                    assert_eq!(plan.added[0].number, 20);
                    model::assert_image(&compose(&source.bytes, &plan), &expected);
                    assert_eq!(image.as_bytes(), source.bytes);
                }
            }
        }
    }
}

#[test]
fn prior_data_reservation_and_new_index_share_the_same_global_byte() {
    for right in [false, true] {
        let source = model::source(6, true, right, false);
        let check = checked(&source.bytes, true);
        let image = capture(&source.bytes);
        let keys = model::final_keys(&source, true, false);
        let mut expected = model::expected(&source, &keys, true);
        for row in [4027, 3958] {
            expected[11 * 4096 + row + 5 + 20 / 8] |= 1 << (20 % 8);
        }
        let plan = continue_primary(
            operation(&image, &check, &keys),
            seeded(&image),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.added[0].number, 21);
        assert_eq!(plan.append.reservations_len(), 2);
        plan.append
            .require_data(20, &mut budget(), &|| false)
            .unwrap();
        model::assert_image(&compose(&source.bytes, &plan), &expected);
    }
}

#[test]
fn distant_child_allocation_keeps_bitmap_metadata_outside_index_members() {
    for code in [5, 6] {
        for right in [false, true] {
            let source = model::source(code, false, right, true);
            let check = checked(&source.bytes, false);
            let image = capture(&source.bytes);
            let keys = model::final_keys(&source, false, false);
            let expected = model::expected(&source, &keys, false);
            let plan = continue_primary(
                operation(&image, &check, &keys),
                State::new(&image, 10),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(plan.append.indexes().count(), 1);
            assert_eq!(plan.promotions.len(), 1);
            assert_eq!(plan.promotions[0].number, 601);
            assert_eq!(
                plan.added
                    .iter()
                    .map(|page| page.number)
                    .collect::<Vec<_>>(),
                [600, 601]
            );
            model::assert_image(&compose(&source.bytes, &plan), &expected);
        }
    }
}

#[test]
fn zero_overflow_changed_source_pairs_duplicate_rows_and_nonfree_eof_refuse() {
    let source = model::source(5, true, false, false);
    let check = checked(&source.bytes, true);
    let image = capture(&source.bytes);
    let mut fitting = source.keys.clone();
    fitting.push(KeyRef {
        key: Some(2),
        row: (15 << 8) | 197,
    });
    assert_eq!(
        continue_primary(
            operation(&image, &check, &fitting),
            State::new(&image, 10),
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    let mut changed = model::final_keys(&source, false, false);
    changed[0].row = (19 << 8) | 1;
    assert!(
        continue_primary(
            operation(&image, &check, &changed),
            State::new(&image, 10),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    let mut duplicate = model::final_keys(&source, false, false);
    duplicate[1].row = duplicate[0].row;
    assert_eq!(
        continue_primary(
            operation(&image, &check, &duplicate),
            State::new(&image, 10),
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Conflict)
    );
    let mut nonfree = source.bytes.clone();
    nonfree[4096 + 4027 + 5 + 20 / 8] &= !(1 << (20 % 8));
    let nonfree_image = capture(&nonfree);
    let keys = model::final_keys(&source, false, false);
    assert!(
        continue_primary(
            operation(&nonfree_image, &check, &keys),
            State::new(&nonfree_image, 10),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn quotas_and_late_cancellation_leave_source_retryable() {
    let source = model::source(6, true, true, false);
    let check = checked(&source.bytes, true);
    let image = capture(&source.bytes);
    let keys = model::final_keys(&source, false, true);
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut allowance = budget();
        allowance
            .charge(resource, allowance.remaining(resource))
            .unwrap();
        assert!(
            continue_primary(
                operation(&image, &check, &keys),
                State::new(&image, 10),
                &mut allowance,
                &|| false
            )
            .is_err()
        );
    }
    let calls = Cell::new(0);
    assert!(
        continue_primary(
            operation(&image, &check, &keys),
            State::new(&image, 10),
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() > 400
            }
        )
        .is_err()
    );
    let plan = continue_primary(
        operation(&image, &check, &keys),
        State::new(&image, 10),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    model::assert_image(
        &compose(&source.bytes, &plan),
        &model::expected(&source, &keys, false),
    );
    assert_eq!(image.as_bytes(), source.bytes);
}
