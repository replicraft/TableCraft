//! Immutable complete images and correlated-plan authority controls.
use super::super::joint_split_tests::model::{self, Case, Family, Shape};
use super::{finish, prepare};
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
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

fn name(row: &model::Row, family: Family) -> Value {
    match &row.name {
        None => Value::Null,
        Some(bytes) if matches!(family, Family::Binary) => Value::Binary(bytes.clone()),
        Some(bytes) => Value::Text(String::from_utf8(bytes.clone()).unwrap()),
    }
}

fn requests(case: &Case) -> (Vec<FixedScalarUpdate>, Vec<SlotScalarUpdate>) {
    let fixed = case
        .rows
        .iter()
        .zip(&case.final_rows)
        .enumerate()
        .map(|(row, (before, after))| {
            FixedScalarUpdate::new(row, Value::Int32(before.age), Value::Int32(after.age))
        })
        .collect();
    let variable = case
        .rows
        .iter()
        .zip(&case.final_rows)
        .enumerate()
        .map(|(row, (before, after))| {
            SlotScalarUpdate::new(row, name(before, case.family), name(after, case.family))
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

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let differences = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        actual == expected,
        "whole image lengths {}/{}, first {first:?}, differences {differences}",
        actual.len(),
        expected.len()
    );
}

#[test]
fn all_prefrozen_joint_images_match_both_group_orders() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for shape in [Shape::Root, Shape::Branch] {
                    let case = model::case(code, family, descending, shape);
                    let (fixed, variable) = requests(&case);
                    let mut groups = groups(&fixed, &variable);

                    for _ in 0..2 {
                        let candidate = capture(&case.before)
                            .preview_ace16_mixed_split_updates(
                                "Items",
                                &groups,
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                        same_image(candidate.before_bytes(), &case.before);
                        same_image(candidate.candidate_bytes(), &case.expected);
                        assert_eq!(candidate.changed_rows(), 64);
                        groups.reverse();
                    }
                }
            }
        }
    }
}

#[test]
fn selected_noops_keep_the_complete_original_topology() {
    for shape in [Shape::Root, Shape::Branch] {
        let case = model::case(6, Family::Modern, true, shape);
        let fixed = case
            .rows
            .iter()
            .enumerate()
            .map(|(row, original)| {
                FixedScalarUpdate::new(row, Value::Int32(original.age), Value::Int32(original.age))
            })
            .collect::<Vec<_>>();
        let variable = case
            .rows
            .iter()
            .enumerate()
            .map(|(row, original)| {
                SlotScalarUpdate::new(
                    row,
                    name(original, case.family),
                    name(original, case.family),
                )
            })
            .collect::<Vec<_>>();
        let candidate = capture(&case.before)
            .preview_ace16_mixed_split_updates(
                "Items",
                &groups(&fixed, &variable),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same_image(candidate.candidate_bytes(), &case.before);
        assert_eq!(candidate.changed_rows(), 0);
    }
}

#[test]
fn late_original_expectation_refuses_before_any_candidate() {
    let case = model::case(6, Family::Modern, false, Shape::Branch);
    let (fixed, mut variable) = requests(&case);
    variable[63] = SlotScalarUpdate::new(63, Value::Null, name(&case.final_rows[63], case.family));
    assert!(matches!(
        capture(&case.before).preview_ace16_mixed_split_updates(
            "Items",
            &groups(&fixed, &variable),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Conflict)
    ));
}

#[test]
fn all_exact_pools_and_final_callback_are_enforced() {
    let case = model::case(6, Family::Legacy, true, Shape::Branch);
    let (fixed, variable) = requests(&case);
    let groups = groups(&fixed, &variable);
    let mut work = budget();
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let before = pools.map(|pool| work.remaining(pool));
    let polls = Cell::new(0);
    let candidate = capture(&case.before)
        .preview_ace16_mixed_split_updates("Items", &groups, &mut work, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    same_image(candidate.candidate_bytes(), &case.expected);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| before[index] - work.remaining(pools[index]));

    for index in 0..3 {
        let mut limits = [512 * 1024 * 1024, 1_000_000, 500_000_000];
        limits[index] = needed[index];
        let candidate = capture(&case.before)
            .preview_ace16_mixed_split_updates(
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
                .preview_ace16_mixed_split_updates(
                    "Items",
                    &groups,
                    &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                    &|| false
                )
                .is_err()
        );
    }

    let final_poll = polls.get();
    let calls = Cell::new(0);
    assert!(matches!(
        capture(&case.before).preview_ace16_mixed_split_updates(
            "Items",
            &groups,
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == final_poll
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn correlated_record_and_joint_keys_cannot_replace_original_groups() {
    for equivalent_text in [false, true] {
        let case = model::case(6, Family::Legacy, false, Shape::Root);
        let (fixed, variable) = requests(&case);
        let original = groups(&fixed, &variable);
        let (mut forged_fixed, mut forged_variable) = requests(&case);
        if equivalent_text {
            forged_variable[0] = SlotScalarUpdate::new(0, Value::Null, Value::Text("A".repeat(80)));
        } else {
            forged_fixed[0] = FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(9999));
        }
        let image = capture(&case.before);
        let staged = prepare(
            &image,
            "Items",
            &groups(&forged_fixed, &forged_variable),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            finish::finish(image, "Items", &original, &staged, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
    }
}

#[test]
fn unselected_payload_and_unused_global_bits_are_not_patch_authority() {
    for forge_map in [false, true] {
        let case = model::case(6, Family::Legacy, false, Shape::Root);
        let (fixed, variable) = requests(&case);
        let groups = groups(&fixed, &variable);
        let image = capture(&case.before);
        let mut staged = prepare(&image, "Items", &groups, &mut budget(), &|| false).unwrap();
        if forge_map {
            // The root case ends at page23; its source-free bit is adjacent to
            // the legitimate shared DATA/INDEX allocations in the same byte.
            let map_start = usize::from(u16::from_le_bytes(
                case.before[4096 + 14..4096 + 16].try_into().unwrap(),
            ));
            let global_byte = 4096 + map_start + 5 + 23 / 8;
            let edit = staged
                .indexes
                .append
                .map_edits
                .iter_mut()
                .find(|edit| edit.offset == global_byte)
                .unwrap();
            assert_ne!(edit.before & 0x80, 0);
            assert_ne!(edit.after & 0x80, 0);
            edit.after &= !0x80;
        } else {
            let target = staged.data.events[0].target.clone();
            let page = staged
                .data
                .pages
                .iter_mut()
                .find(|page| page.number as usize == target.start / 4096)
                .unwrap();
            let offset = target.start % 4096;
            page.after[offset + 2..offset + 6].copy_from_slice(&777_i32.to_le_bytes());
        }
        assert!(matches!(
            finish::finish(image, "Items", &groups, &staged, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
    }
}
