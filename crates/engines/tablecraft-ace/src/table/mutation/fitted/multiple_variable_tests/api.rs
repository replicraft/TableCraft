//! Complete frozen images and original-request controls for whole-row updates.
use super::super::{record_proof, stage};
use super::{budget, model, third_model};
use crate::{
    Ace16WritePreview, EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError,
    NativeImage, SlotScalarUpdate, WriteBlocker,
    table::{indexes::index_row_patches, mutation::gates},
};
use model::{Case, Family, Placement, Recipe};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

struct Requests {
    fixed: Vec<FixedScalarUpdate>,
    first: Vec<SlotScalarUpdate>,
    second: Vec<SlotScalarUpdate>,
    third: Vec<SlotScalarUpdate>,
}

impl Requests {
    fn new(case: &Case, third: bool) -> Self {
        let variable = |rows: &[usize], column: usize| -> Vec<SlotScalarUpdate> {
            rows.iter()
                .map(|&row| {
                    SlotScalarUpdate::new(
                        row,
                        model::values(&case.rows[row], case.family)[column].clone(),
                        model::values(&case.final_rows[row], case.family)[column].clone(),
                    )
                })
                .collect()
        };
        Self {
            fixed: case
                .fixed_rows
                .iter()
                .map(|&row| {
                    FixedScalarUpdate::new(
                        row,
                        case.rows[row].age.map_or(Value::Null, Value::Int32),
                        case.final_rows[row].age.map_or(Value::Null, Value::Int32),
                    )
                })
                .collect(),
            first: variable(&case.first_rows, 2),
            second: variable(&case.second_rows, 3),
            third: if third {
                variable(&[0, 3], 4)
            } else {
                Vec::new()
            },
        }
    }

    fn reverse_rows(&mut self) {
        self.fixed.reverse();
        self.first.reverse();
        self.second.reverse();
        self.third.reverse();
    }

    fn groups(&self) -> Vec<FittedColumnUpdates<'_>> {
        let mut groups = Vec::new();
        if !self.fixed.is_empty() {
            groups.push(FittedColumnUpdates::Fixed {
                column: "Age",
                updates: &self.fixed,
            });
        }
        groups.push(FittedColumnUpdates::Variable {
            column: "First",
            updates: &self.first,
        });
        groups.push(FittedColumnUpdates::Variable {
            column: "Second",
            updates: &self.second,
        });
        if !self.third.is_empty() {
            groups.push(FittedColumnUpdates::Variable {
                column: "Opaque",
                updates: &self.third,
            });
        }
        groups
    }
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

fn run(
    case: &Case,
    groups: &[FittedColumnUpdates<'_>],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image = capture(&case.before);
    match case.placement {
        Placement::Fitted => image.preview_ace16_fitted_updates("Items", groups, work, cancelled),
        Placement::Data => image.preview_ace16_mixed_updates("Items", groups, work, cancelled),
        Placement::DataIndexes => {
            image.preview_ace16_mixed_split_updates("Items", groups, work, cancelled)
        }
    }
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

fn all_orders(case: Case, third: bool) {
    let mut requests = Requests::new(&case, third);
    for _ in 0..2 {
        for reverse in [false, true] {
            let mut groups = requests.groups();
            if reverse {
                groups.reverse();
            }
            let result = run(&case, &groups, &mut budget(), &|| false).unwrap();
            same_image(result.before_bytes(), &case.before);
            same_image(result.candidate_bytes(), &case.expected);
            assert_eq!(result.changed_rows(), case.changed.len());
        }
        requests.reverse_rows();
    }
}

#[test]
fn whole_statement_multiple_variables_match_all36_frozen_pairs_in_all_orders() {
    for code in [5, 6] {
        for family in [Family::LegacyText, Family::ModernBinary, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::NullShrinkGrow] {
                    all_orders(model::fitted(code, family, descending, recipe), false);
                }
            }
        }
        for family in [Family::LegacyBinary, Family::ModernBinary] {
            for index_growth in [false, true] {
                all_orders(model::growing(code, family, index_growth), false);
            }
        }
        for exceeds in [false, true] {
            all_orders(model::capacity(code, exceeds), false);
        }
    }
}

#[test]
fn three_variable_updates_compose_overlapping_and_disjoint_rows_once() {
    for code in [5, 6] {
        for family in [Family::LegacyText, Family::ModernBinary] {
            for descending in [false, true] {
                all_orders(third_model::case(code, family, descending), true);
            }
        }
    }
}

#[test]
fn late_original_expectation_and_duplicate_intents_refuse_atomically() {
    let case = third_model::case(6, Family::ModernBinary, true);
    let mut requests = Requests::new(&case, true);
    requests.third[1] = SlotScalarUpdate::new(3, Value::Binary(vec![0]), Value::Binary(vec![1]));
    assert!(matches!(
        run(&case, &requests.groups(), &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));

    let requests = Requests::new(&case, true);
    let mut groups = requests.groups();
    groups.push(FittedColumnUpdates::Variable {
        column: "FIRST",
        updates: &requests.first,
    });
    assert!(matches!(
        run(&case, &groups, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));

    let mut requests = Requests::new(&case, true);
    let value = model::values(&case.rows[0], case.family)[2].clone();
    requests
        .first
        .push(SlotScalarUpdate::new(0, value.clone(), value));
    assert!(matches!(
        run(&case, &requests.groups(), &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
}

#[test]
fn fitting_capacity_is_joint_and_all_noop_raw_spans_remain_exact() {
    let case = model::capacity(6, true);
    let requests = Requests::new(&case, false);
    assert!(matches!(
        capture(&case.before).preview_ace16_fitted_updates(
            "Items",
            &requests.groups(),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));

    let case = model::fitted(6, Family::LegacyText, false, Recipe::NullShrinkGrow);
    let unchanged = (0..case.rows.len())
        .map(|row| {
            let values = model::values(&case.rows[row], case.family);
            SlotScalarUpdate::new(row, values[2].clone(), values[2].clone())
        })
        .collect::<Vec<_>>();
    let groups = [
        FittedColumnUpdates::Variable {
            column: "First",
            updates: &unchanged,
        },
        FittedColumnUpdates::Variable {
            column: "Second",
            updates: &[],
        },
    ];
    let result = run(&case, &groups, &mut budget(), &|| false).unwrap();
    same_image(result.candidate_bytes(), &case.before);
    assert_eq!(result.changed_rows(), 0);
}

#[test]
fn joint_composition_and_proof_pay_exact_pools_and_final_cancellation() {
    for case in [
        third_model::case(6, Family::LegacyText, true),
        model::growing(6, Family::ModernBinary, false),
        model::growing(6, Family::ModernBinary, true),
    ] {
        let requests = Requests::new(&case, matches!(case.placement, Placement::Fitted));
        let groups = requests.groups();
        let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
        let limits = [512 * 1024 * 1024, 1_000_000, 500_000_000];
        let mut work = ResourceBudget::new(limits[0], limits[1], limits[2]);
        let polls = Cell::new(0);
        let result = run(&case, &groups, &mut work, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        same_image(result.candidate_bytes(), &case.expected);
        for index in 0..3 {
            let needed = limits[index] - work.remaining(resources[index]);
            assert!(needed > 0);
            let mut exact = limits;
            exact[index] = needed;
            run(
                &case,
                &groups,
                &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                &|| false,
            )
            .unwrap();
            exact[index] -= 1;
            assert!(matches!(
                run(
                    &case,
                    &groups,
                    &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                    &|| false
                ),
                Err(EditError::Inspect(InspectError::Limit(_)))
            ));
        }
        let stop = polls.get();
        let calls = Cell::new(0);
        assert!(matches!(
            run(&case, &groups, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
}

#[test]
fn correlated_selected_payload_and_joint_key_caches_cannot_replace_original_groups() {
    let case = model::fitted(6, Family::LegacyText, false, Recipe::NullShrinkGrow);
    let requests = Requests::new(&case, false);
    let groups = requests.groups();
    let image = capture(&case.before);
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let prove_forgery = |staged: &stage::Staged| {
        let keys = staged.keys(&mut budget(), &|| false).unwrap();
        let indexes = index_row_patches(
            &image,
            checked.schema(),
            checked.indexes(),
            &staged.affected,
            &keys,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            record_proof::prove(
                &image,
                "Items",
                &groups,
                staged,
                &indexes,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(EditError::Preservation)
        );
    };

    let mut alternate = Requests::new(&case, false);
    alternate.fixed[0] = FixedScalarUpdate::new(2, Value::Null, Value::Int32(99));
    let staged = stage::build(
        &image,
        &checked,
        &alternate.groups(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    prove_forgery(&staged);

    // A selected variable payload and its complete tuple cache/INDEX ledger
    // agree with another real operation, but not the original statement.
    let mut alternate = Requests::new(&case, false);
    alternate.second[0] = SlotScalarUpdate::new(
        2,
        model::values(&case.rows[2], case.family)[3].clone(),
        Value::Text("x".into()),
    );
    let staged = stage::build(
        &image,
        &checked,
        &alternate.groups(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    prove_forgery(&staged);

    // An unselected third-slot payload has no key effect. Complete-record
    // authority must still reject it when every staged key remains coherent.
    let mut staged = stage::build(&image, &checked, &groups, &mut budget(), &|| false).unwrap();
    let after = &mut staged.records[0].after;
    let footer = after.len() - 11;
    let opaque = usize::from(u16::from_le_bytes(
        after[footer + 2..footer + 4].try_into().unwrap(),
    ));
    assert_eq!(after[opaque], 0xde);
    after[opaque] = 0x42;
    prove_forgery(&staged);
}
