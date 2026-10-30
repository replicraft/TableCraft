//! Private future child of the ACTUAL mixed_fitted_tests literal owner.
use super::{budget, model, qualify};
use crate::table::indexes::{RowKeyUpdate, index_row_patches};
use crate::{EditError, NativeImage, check_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const COLUMNS: [usize; 2] = [1, 2];

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn projections(rows: &[model::Row], family: model::Family) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| {
            vec![
                row.age.map_or(Value::Null, Value::Int32),
                model::name_value(row, family),
            ]
        })
        .collect()
}

#[test]
fn joint_row_keys_match_every_frozen_physical_owner_once() {
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for recipe in [
                model::Recipe::Permutation,
                model::Recipe::Shorter,
                model::Recipe::Nulls,
            ] {
                let case = model::case(code, family, false, recipe, false);
                qualify(&case.before, &case.rows, family);
                qualify(&case.expected, &case.final_rows, family);
                let image = capture(&case.before);
                let checked = check_table(
                    &mut image.reader(),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let original = projections(&case.rows, family);
                let final_values = projections(&case.final_rows, family);
                let updates = case
                    .changed
                    .iter()
                    .map(|&row| RowKeyUpdate {
                        reference: (12 << 8) | row as u32,
                        columns: &COLUMNS,
                        before: &original[row],
                        after: &final_values[row],
                    })
                    .collect::<Vec<_>>();
                let patches = index_row_patches(
                    &image,
                    checked.schema(),
                    checked.indexes(),
                    &COLUMNS,
                    &updates,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let mut composed = case.before.clone();
                for patch in patches {
                    assert_eq!(
                        &case.before[patch.offset..patch.offset + patch.before.len()],
                        patch.before
                    );
                    composed[patch.offset..patch.offset + patch.after.len()]
                        .copy_from_slice(&patch.after);
                }
                // This primitive returns INDEX effects only, not a candidate
                // record or full-image preservation certificate.
                for page in [13, 15, 16] {
                    assert_eq!(
                        &composed[page * 4096..(page + 1) * 4096],
                        &case.expected[page * 4096..(page + 1) * 4096]
                    );
                }
                assert_eq!(
                    &composed[12 * 4096..13 * 4096],
                    &case.before[12 * 4096..13 * 4096]
                );
                assert_eq!(composed.len(), case.before.len());
            }
        }
    }
}

#[test]
fn altered_original_key_and_missing_logical_row_refuse_instead_of_becoming_authority() {
    let case = model::case(
        5,
        model::Family::Legacy,
        false,
        model::Recipe::Permutation,
        false,
    );
    qualify(&case.before, &case.rows, case.family);
    let image = capture(&case.before);
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let original = projections(&case.rows, case.family);
    let after = projections(&case.final_rows, case.family);
    let mut forged_original = original[0].clone();
    forged_original[0] = Value::Int32(999);
    let updates = [RowKeyUpdate {
        reference: 12 << 8,
        columns: &COLUMNS,
        before: &forged_original,
        after: &after[0],
    }];
    assert!(matches!(
        index_row_patches(
            &image,
            checked.schema(),
            checked.indexes(),
            &COLUMNS,
            &updates,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Conflict)
    ));

    let updates = [RowKeyUpdate {
        reference: 12 << 8 | 200,
        columns: &COLUMNS,
        before: &original[0],
        after: &after[0],
    }];
    assert!(matches!(
        index_row_patches(
            &image,
            checked.schema(),
            checked.indexes(),
            &COLUMNS,
            &updates,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
}
