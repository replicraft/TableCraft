//! Prefrozen original images, joint updates and actual authority forgeries.
mod boundary;

use super::super::{
    record_proof, request::FittedColumnUpdates, serialization::RecordPolicy, stage as record_stage,
};
use super::{finish, stage};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate, WriteBlocker,
    table::{
        indexes::{IndexDataPatch, index_row_patches},
        mutation::{
            gates,
            mixed_fitted_tests::{growing as model, model::Family},
        },
    },
};
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

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    let count = actual
        .iter()
        .zip(expected)
        .filter(|(left, right)| left != right)
        .count();
    assert!(
        actual == expected,
        "whole image mismatch: lengths {}/{}, first {:?}, differences {}",
        actual.len(),
        expected.len(),
        first,
        count
    );
}

fn requests(case: &model::Case) -> (Vec<FixedScalarUpdate>, Vec<SlotScalarUpdate>) {
    let fixed = vec![FixedScalarUpdate::new(3, Value::Null, Value::Int32(40))];
    let variable = vec![SlotScalarUpdate::new(
        3,
        Value::Null,
        crate::table::mutation::mixed_fitted_tests::model::name_value(
            &case.final_rows[3],
            case.family,
        ),
    )];
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

fn indexes(image: &NativeImage, prepared: &stage::Prepared) -> Vec<IndexDataPatch> {
    let mut work = budget();
    let keys = prepared.verified.keys(&mut work, &|| false).unwrap();
    index_row_patches(
        image,
        prepared.verified.checked.schema(),
        prepared.verified.checked.indexes(),
        &prepared.verified.columns,
        &keys,
        &mut work,
        &|| false,
    )
    .unwrap()
}

#[test]
fn all_growing_images_match_in_both_original_group_orders() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for destination in [
                    model::Destination::OriginalPage,
                    model::Destination::OtherOwned,
                    model::Destination::EndOfFile,
                ] {
                    let case = model::case(code, family, descending, destination);
                    let (fixed, variable) = requests(&case);
                    let mut groups = groups(&fixed, &variable);

                    for _ in 0..2 {
                        let candidate = capture(&case.before)
                            .preview_ace16_mixed_updates("Items", &groups, &mut budget(), &|| false)
                            .unwrap();
                        same_image(candidate.before_bytes(), &case.before);
                        same_image(candidate.candidate_bytes(), &case.expected);
                        assert_eq!(candidate.changed_rows(), 1);
                        groups.reverse();
                    }
                }
            }
        }
    }
}

#[test]
fn original_noops_and_late_stale_expectations_remain_atomic() {
    let case = model::case(6, Family::Legacy, false, model::Destination::EndOfFile);
    let fixed = [FixedScalarUpdate::new(3, Value::Null, Value::Null)];
    let variable = [SlotScalarUpdate::new(3, Value::Null, Value::Null)];
    let candidate = capture(&case.before)
        .preview_ace16_mixed_updates("Items", &groups(&fixed, &variable), &mut budget(), &|| {
            false
        })
        .unwrap();
    same_image(candidate.candidate_bytes(), &case.before);
    assert_eq!(candidate.changed_rows(), 0);

    let (fixed, mut variable) = requests(&case);
    variable[0] = SlotScalarUpdate::new(3, Value::Text("stale".into()), Value::Text("aa".into()));
    assert!(matches!(
        capture(&case.before).preview_ace16_mixed_updates(
            "Items",
            &groups(&fixed, &variable),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Conflict),
    ));
}

#[test]
fn growth_respects_all_exact_pools_and_the_final_poll() {
    let case = model::case(5, Family::Modern, true, model::Destination::EndOfFile);
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut work = budget();
    let before = pools.map(|pool| work.remaining(pool));
    let polls = Cell::new(0);
    let candidate = capture(&case.before)
        .preview_ace16_mixed_updates("Items", &groups, &mut work, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    same_image(candidate.candidate_bytes(), &case.expected);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| before[index] - work.remaining(pools[index]));

    for index in 0..3 {
        let mut limits = [256 * 1024 * 1024, 100_000, 100_000_000];
        limits[index] = needed[index];
        let candidate = capture(&case.before)
            .preview_ace16_mixed_updates(
                "Items",
                &groups,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            )
            .unwrap();
        same_image(candidate.candidate_bytes(), &case.expected);
        limits[index] -= 1;
        assert!(
            capture(&case.before)
                .preview_ace16_mixed_updates(
                    "Items",
                    &groups,
                    &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                    &|| false,
                )
                .is_err()
        );
    }

    let stop = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        capture(&case.before).preview_ace16_mixed_updates("Items", &groups, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == stop
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn reader_valid_index_body_alias_refuses_the_actual_api() {
    let case = model::case(6, Family::Legacy, false, model::Destination::OriginalPage);
    let mut aliased = case.before.clone();
    let page = 14 * 4096;
    let start = usize::from(u16::from_le_bytes(
        aliased[page + 14..page + 16].try_into().unwrap(),
    ));
    aliased[page + start + 5 + 12 / 8] |= 1 << (12 % 8);
    let (fixed, variable) = requests(&case);
    assert!(matches!(
        capture(&aliased).preview_ace16_mixed_updates(
            "Items",
            &groups(&fixed, &variable),
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn correlated_final_record_and_keys_do_not_replace_original_groups() {
    for unselected in [false, true] {
        let case = model::case(6, Family::Legacy, false, model::Destination::EndOfFile);
        let (fixed, variable) = requests(&case);
        let groups = groups(&fixed, &variable);
        let image = capture(&case.before);
        let mut prepared =
            stage::prepare(&image, "Items", &groups, &mut budget(), &|| false).unwrap();
        let target = prepared.placement.view().events[0].target.clone();
        let page = prepared
            .placement
            .pages
            .iter_mut()
            .find(|page| page.number as usize == target.start / 4096)
            .unwrap();
        let start = target.start % 4096;

        if unselected {
            // Name is four raw bytes; unselected Other begins at byte14.
            page.after[start + 14] = b'Q';
        } else {
            page.after[start + 6..start + 10].copy_from_slice(&99_i32.to_le_bytes());
            prepared.verified.records[0].bytes[6..10].copy_from_slice(&99_i32.to_le_bytes());
            let age = prepared
                .verified
                .projection
                .iter()
                .position(|&column| column == 1)
                .unwrap();
            prepared.verified.records[0].final_values[age] = Value::Int32(99);
        }
        let patches = indexes(&image, &prepared);
        assert!(matches!(
            finish::finish(
                image,
                "Items",
                &groups,
                &prepared,
                &patches,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
    }
}

#[test]
fn agreeing_extra_global_claim_is_not_a_placement_event() {
    let case = model::case(6, Family::Legacy, false, model::Destination::EndOfFile);
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let image = capture(&case.before);
    let mut prepared = stage::prepare(&image, "Items", &groups, &mut budget(), &|| false).unwrap();
    let global = prepared
        .placement
        .append
        .map_edits
        .iter_mut()
        .find(|edit| (4096..8192).contains(&edit.offset))
        .unwrap();
    assert_ne!(global.after & 4, 0);
    global.after &= !4;
    let patches = indexes(&image, &prepared);
    assert!(matches!(
        finish::finish(
            image,
            "Items",
            &groups,
            &prepared,
            &patches,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
}

#[test]
fn collation_equivalent_producer_projection_is_not_exact_record_authority() {
    let case = model::case(6, Family::Legacy, false, model::Destination::OriginalPage);
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let image = capture(&case.before);
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut staged = record_stage::build_with(
        &image,
        &checked,
        &groups,
        RecordPolicy::GrowingPreferred,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let name = staged
        .projection
        .iter()
        .position(|&column| column == 2)
        .unwrap();
    staged.records[0].final_values[name] = Value::Text("AA".into());
    let keys = staged.keys(&mut budget(), &|| false).unwrap();
    let patches = index_row_patches(
        &image,
        checked.schema(),
        checked.indexes(),
        &staged.affected,
        &keys,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let fresh = record_proof::records(
        &image,
        "Items",
        &groups,
        RecordPolicy::GrowingPreferred,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    // Physical collation permits the same keys; exact source/value proof does not.
    fresh
        .prove_indexes(&image, &patches, &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        fresh.compare_staged(&staged, &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));
}
