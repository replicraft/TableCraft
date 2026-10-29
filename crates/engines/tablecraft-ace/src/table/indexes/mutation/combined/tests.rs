//! Direct physical-plan controls, separate from cascade orchestration.
mod model;

use super::*;
use crate::table::indexes::composite::DecimalValue;
use crate::{IndexCheckStatus, IndexConstraintViolation, TableCheck, check_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn captured(bytes: &[u8]) -> (NativeImage, TableCheck) {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    (image, checked)
}

fn update(kind: u8, slot: u32) -> IndexKeyUpdate {
    let value = slot + 1;
    IndexKeyUpdate {
        reference: (12 << 8) | slot,
        column: 1,
        expected: Some(match kind {
            15 => DecimalValue::Guid([value as u8; 16]),
            16 => DecimalValue::Decimal(i128::from(value)),
            _ => DecimalValue::Numeric(i64::from(value)),
        }),
        value: None,
    }
}

fn run(
    image: &NativeImage,
    checked: &TableCheck,
    deleted: &[u32],
    updates: &[IndexKeyUpdate],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PatchSet, EditError> {
    index_delete_update_patches(
        image,
        checked.schema(),
        checked.indexes(),
        deleted,
        updates,
        work,
        cancelled,
    )
}

fn apply(bytes: &[u8], plan: &PatchSet) -> Vec<u8> {
    assert!(plan.retired.is_empty());
    let mut after = bytes.to_vec();
    for patch in &plan.indexes {
        let end = patch.offset + patch.before.len();
        assert_eq!(&after[patch.offset..end], patch.before);
        after[patch.offset..end].copy_from_slice(&patch.after);
    }
    after
}

#[test]
fn qualify_combined_fixed_width_sources_and_literal_expected_images() {
    for code in [5, 6] {
        for kind in [2, 3, 4, 5, 19, 15, 16] {
            for descending in [false, true] {
                for ignored in [false, true] {
                    let before =
                        model::source(code, kind, descending, if ignored { 0x82 } else { 0x80 });
                    captured(&before);
                    let mut entries = vec![model::key(kind, Some(2), (12 << 8) | 1, descending)];
                    if !ignored {
                        entries.push(model::key(kind, None, (12 << 8) | 2, descending));
                    }
                    let expected =
                        model::expected_root(&before[14 * model::PAGE..15 * model::PAGE], entries);
                    let mut after = before;
                    after[14 * model::PAGE..15 * model::PAGE].copy_from_slice(&expected);
                    model::apply_intents(&mut after, &[0], &[2]);
                    assert_eq!(captured(&after).1.rows(), 2);
                }
            }
        }
    }
    captured(&model::source(6, 4, false, 0x8a));
    assert_eq!(captured(&model::pressure_source()).1.rows(), 451);
    let mut empty = model::source(6, 4, false, 0x82);
    let root = model::expected_root(&empty[14 * model::PAGE..15 * model::PAGE], Vec::new());
    empty[14 * model::PAGE..15 * model::PAGE].copy_from_slice(&root);
    model::apply_intents(&mut empty, &[], &[0, 1, 2]);
    assert_eq!(captured(&empty).1.rows(), 3);
}

#[test]
fn mixed_deletion_and_null_keys_preserve_every_fixed_width_and_direction() {
    for code in [5, 6] {
        for kind in [2, 3, 4, 5, 19, 15, 16] {
            for descending in [false, true] {
                for ignored in [false, true] {
                    let before =
                        model::source(code, kind, descending, if ignored { 0x82 } else { 0x80 });
                    let mut entries = vec![model::key(kind, Some(2), (12 << 8) | 1, descending)];
                    if !ignored {
                        entries.push(model::key(kind, None, (12 << 8) | 2, descending));
                    }
                    let expected =
                        model::expected_root(&before[14 * model::PAGE..15 * model::PAGE], entries);
                    let (image, checked) = captured(&before);
                    let plan = run(
                        &image,
                        &checked,
                        &[12 << 8],
                        &[update(kind, 2)],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let mut after = apply(&before, &plan);
                    assert_eq!(&after[14 * model::PAGE..15 * model::PAGE], expected);
                    model::apply_intents(&mut after, &[0], &[2]);
                    let (_, checked_after) = captured(&after);
                    assert_eq!(checked_after.rows(), 2);
                }
            }
        }
    }
}

#[test]
fn required_null_is_checked_before_ignore_null_omission() {
    let before = model::source(6, 4, false, 0x8a);
    let (image, checked) = captured(&before);
    assert!(matches!(
        run(
            &image,
            &checked,
            &[],
            &[update(4, 2)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey
        )))
    ));
}

#[test]
fn ignored_all_null_result_retains_root_storage_and_table_membership() {
    let before = model::source(6, 4, false, 0x82);
    let expected = model::expected_root(&before[14 * model::PAGE..15 * model::PAGE], Vec::new());
    let (image, checked) = captured(&before);
    let updates = [update(4, 0), update(4, 1), update(4, 2)];
    let plan = run(&image, &checked, &[], &updates, &mut budget(), &|| false).unwrap();
    let mut after = apply(&before, &plan);
    assert_eq!(&after[14 * model::PAGE..15 * model::PAGE], expected);
    model::apply_intents(&mut after, &[], &[0, 1, 2]);
    let (_, checked_after) = captured(&after);
    assert_eq!(checked_after.rows(), 3);
}

#[test]
fn duplicate_overlap_stale_and_non_null_update_intents_refuse() {
    let before = model::source(6, 4, false, 0x80);
    let (image, checked) = captured(&before);
    assert!(
        run(
            &image,
            &checked,
            &[(12 << 8) | 2],
            &[update(4, 2)],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        run(
            &image,
            &checked,
            &[],
            &[update(4, 2), update(4, 2)],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        run(
            &image,
            &checked,
            &[12 << 8, 12 << 8],
            &[update(4, 2)],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for mutation in 0..5 {
        let mut intent = update(4, 2);
        match mutation {
            0 => intent.expected = None,
            1 => intent.value = Some(DecimalValue::Numeric(9)),
            2 => intent.expected = Some(DecimalValue::Numeric(4)),
            3 => intent.expected = Some(DecimalValue::Guid([3; 16])),
            4 => intent.column = 2,
            _ => unreachable!(),
        }
        assert!(run(&image, &checked, &[], &[intent], &mut budget(), &|| false).is_err());
    }
}

#[test]
fn prefix_loss_overflow_refuses_without_a_partial_physical_plan() {
    let before = model::pressure_source();
    let (image, checked) = captured(&before);
    assert_eq!(checked.rows(), 451);
    assert!(matches!(
        run(
            &image,
            &checked,
            &[],
            &[update(4, 0)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
}

#[test]
fn physical_transform_charges_work_and_terminal_cancellation_returns_no_plan() {
    let before = model::source(6, 15, true, 0x80);
    let (image, checked) = captured(&before);
    let updates = [update(15, 2)];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(run(&image, &checked, &[], &updates, &mut limited, &|| false).is_err());
    }
    let polls = Cell::new(0);
    run(&image, &checked, &[], &updates, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let terminal = polls.get();
    assert!(terminal > 1);
    let polls = Cell::new(0);
    assert!(
        run(&image, &checked, &[], &updates, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() >= terminal
        })
        .is_err()
    );
    assert_eq!(polls.get(), terminal);
}
