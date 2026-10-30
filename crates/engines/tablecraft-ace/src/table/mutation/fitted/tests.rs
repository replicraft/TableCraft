//! Whole-image controls and correlated forgeries use the actual source owner.
mod capacity;
#[path = "../mixed_fitted_tests/framed.rs"]
mod framed;
mod qualified;

use super::{record_proof, request::FittedColumnUpdates, stage};
use crate::table::mutation::{gates, mixed_fitted_tests::model};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate, TableCheck,
    table::indexes::{IndexDataPatch, index_row_patches},
};
use model::{Case, Family, Recipe};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

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

fn requests(case: &Case) -> (Vec<FixedScalarUpdate>, Vec<SlotScalarUpdate>) {
    let fixed = case
        .changed
        .iter()
        .map(|&row| {
            FixedScalarUpdate::new(
                row,
                case.rows[row].age.map_or(Value::Null, Value::Int32),
                case.final_rows[row].age.map_or(Value::Null, Value::Int32),
            )
        })
        .collect();
    let variable = case
        .changed
        .iter()
        .map(|&row| {
            SlotScalarUpdate::new(
                row,
                model::name_value(&case.rows[row], case.family),
                model::name_value(&case.final_rows[row], case.family),
            )
        })
        .collect();
    (fixed, variable)
}

fn groups<'a>(
    fixed: &'a [FixedScalarUpdate],
    variable: &'a [SlotScalarUpdate],
) -> [FittedColumnUpdates<'a>; 2] {
    [
        FittedColumnUpdates::Fixed {
            column: "Age",
            updates: fixed,
        },
        FittedColumnUpdates::Variable {
            column: "Name",
            updates: variable,
        },
    ]
}

fn indexes(
    image: &NativeImage,
    source: &TableCheck,
    staged: &stage::Staged,
) -> Vec<IndexDataPatch> {
    let mut work = budget();
    let keys = staged.keys(&mut work, &|| false).unwrap();
    index_row_patches(
        image,
        source.schema(),
        source.indexes(),
        &staged.affected,
        &keys,
        &mut work,
        &|| false,
    )
    .unwrap()
}

fn same_indexes(left: &[IndexDataPatch], right: &[IndexDataPatch]) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.offset, right.offset);
        assert_eq!(left.before, right.before);
        assert_eq!(left.after, right.after);
    }
}

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected);
    let differences = actual
        .iter()
        .zip(expected)
        .filter(|(actual, expected)| actual != expected)
        .count();
    assert!(
        actual == expected,
        "whole-image mismatch: actual length {}, expected length {}, first offset {:?}, differing common bytes {}",
        actual.len(),
        expected.len(),
        first,
        differences,
    );
}

fn healthy(
    image: &NativeImage,
    case: &Case,
    groups: &[FittedColumnUpdates<'_>],
) -> (TableCheck, stage::Staged, Vec<IndexDataPatch>) {
    let source = gates::before_slot(image, "Items", &mut budget(), &|| false).unwrap();
    let staged = stage::build(image, &source, groups, &mut budget(), &|| false).unwrap();
    assert_eq!(staged.records.len(), case.changed.len());
    for (record, &row) in staged.records.iter().zip(&case.changed) {
        assert_eq!(record.reference, (12 << 8) | row as u32);
        assert_eq!(record.before, case.records[row]);
        assert_eq!(record.after, case.final_records[row]);
    }

    let indexes = indexes(image, &source, &staged);
    assert_eq!(
        record_proof::prove(
            image,
            "Items",
            groups,
            &staged,
            &indexes,
            &mut budget(),
            &|| false
        )
        .unwrap(),
        case.changed.len()
    );
    (source, staged, indexes)
}

#[test]
fn mixed_joint_updates_match_every_prefrozen_image_in_both_group_orders() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::Shorter, Recipe::Nulls] {
                    let case = framed::framed(model::case(code, family, descending, recipe, false));
                    let (fixed, variable) = requests(&case);
                    let mut groups = groups(&fixed, &variable);
                    for _ in 0..2 {
                        let result = capture(&case.before)
                            .preview_ace16_fitted_updates("Items", &groups, &mut budget(), &|| {
                                false
                            })
                            .unwrap();
                        same_image(result.before_bytes(), &case.before);
                        same_image(result.candidate_bytes(), &case.expected);
                        assert_eq!(result.changed_rows(), case.changed.len());
                        groups.reverse();
                    }
                }
            }
        }
    }
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        let case = framed::framed(model::case(6, family, true, Recipe::Permutation, true));
        let (fixed, variable) = requests(&case);
        let result = capture(&case.before)
            .preview_ace16_fitted_updates(
                "Items",
                &groups(&fixed, &variable),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same_image(result.candidate_bytes(), &case.expected);
    }
}

#[test]
fn late_original_expected_and_physical_capacity_fail_without_a_candidate() {
    let case = framed::framed(model::case(
        6,
        Family::Legacy,
        false,
        Recipe::Permutation,
        false,
    ));
    let (fixed, mut variable) = requests(&case);
    let last = *case.changed.last().unwrap();
    variable[last] = SlotScalarUpdate::new(
        last,
        Value::Text("wrong original".into()),
        model::name_value(&case.final_rows[last], case.family),
    );
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fitted_updates(
                "Items",
                &groups(&fixed, &variable),
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );

    let variable = [SlotScalarUpdate::new(
        0,
        model::name_value(&case.rows[0], case.family),
        Value::Text("a".repeat(64)),
    )];
    assert!(matches!(
        capture(&case.before).preview_ace16_fitted_updates(
            "Items",
            &groups(&fixed, &variable),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(_))
    ));
}

#[test]
fn correlated_wrong_fixed_payload_cache_and_index_are_not_original_intent() {
    let case = framed::framed(model::case(
        6,
        Family::Legacy,
        false,
        Recipe::Permutation,
        false,
    ));
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let image = capture(&case.before);
    let (source, mut staged, _) = healthy(&image, &case, &groups);
    let age = staged
        .projection
        .iter()
        .position(|&column| column == 1)
        .unwrap();

    staged.records[0].after[6..10].copy_from_slice(&99_i32.to_le_bytes());
    staged.records[0].final_values[age] = Value::Int32(99);
    let forged_indexes = indexes(&image, &source, &staged);
    assert_eq!(
        record_proof::prove(
            &image,
            "Items",
            &groups,
            &staged,
            &forged_indexes,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn unselected_other_bytes_are_not_authorized_by_agreeing_key_caches() {
    let case = framed::framed(model::case(
        6,
        Family::Legacy,
        false,
        Recipe::Permutation,
        false,
    ));
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let image = capture(&case.before);
    let (source, mut staged, correct_indexes) = healthy(&image, &case, &groups);
    let record = &mut staged.records[0].after;
    let footer = record.len() - 9;
    let other = usize::from(u16::from_le_bytes(
        record[footer + 2..footer + 4].try_into().unwrap(),
    ));
    assert_eq!(record[other], b'Z');
    record[other] = b'Q';

    let forged_indexes = indexes(&image, &source, &staged);
    same_indexes(&correct_indexes, &forged_indexes);
    assert_eq!(
        record_proof::prove(
            &image,
            "Items",
            &groups,
            &staged,
            &forged_indexes,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn collation_equivalent_original_or_final_cache_is_not_exact_stored_value() {
    let case = framed::framed(model::case(
        6,
        Family::Legacy,
        false,
        Recipe::Permutation,
        false,
    ));
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let image = capture(&case.before);
    for original in [true, false] {
        let (source, mut staged, correct_indexes) = healthy(&image, &case, &groups);
        let name = staged
            .projection
            .iter()
            .position(|&column| column == 2)
            .unwrap();
        let cache = if original {
            &mut staged.records[0].original_values
        } else {
            &mut staged.records[1].final_values
        };
        assert_eq!(cache[name], Value::Text("a".into()));
        cache[name] = Value::Text("A".into());

        let forged_indexes = indexes(&image, &source, &staged);
        same_indexes(&correct_indexes, &forged_indexes);
        assert_eq!(
            record_proof::prove(
                &image,
                "Items",
                &groups,
                &staged,
                &forged_indexes,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn mixed_proof_and_deep_work_respect_exact_pools_and_final_cancellation() {
    let case = framed::framed(model::case(
        6,
        Family::Modern,
        true,
        Recipe::Permutation,
        true,
    ));
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let mut work = budget();
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let before = pools.map(|pool| work.remaining(pool));
    let polls = Cell::new(0);
    let result = capture(&case.before)
        .preview_ace16_fitted_updates("Items", &groups, &mut work, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(result.candidate_bytes(), case.expected);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| before[index] - work.remaining(pools[index]));

    for index in 0..3 {
        let mut limits = [256 * 1024 * 1024, 100_000, 100_000_000];
        limits[index] = needed[index];
        let result = capture(&case.before)
            .preview_ace16_fitted_updates(
                "Items",
                &groups,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            )
            .unwrap();
        assert_eq!(result.candidate_bytes(), case.expected);
        limits[index] -= 1;
        assert!(
            capture(&case.before)
                .preview_ace16_fitted_updates(
                    "Items",
                    &groups,
                    &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                    &|| false
                )
                .is_err()
        );
    }

    let stop = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        capture(&case.before).preview_ace16_fitted_updates(
            "Items",
            &groups,
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}
