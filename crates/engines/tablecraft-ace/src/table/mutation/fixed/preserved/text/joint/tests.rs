//! Literal whole-image acceptance and independent observed-image forgeries.
use super::super::super::{short_text_sources::joint_fixed_sources::model, source};
use super::proof::{self, Observed};
use crate::{
    Ace16WritePreview, ConstraintViolation, EditError, FittedColumnUpdates, FixedScalarUpdate,
    InspectError, NativeImage, SlotScalarUpdate, WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

const PAGE_BYTES: usize = 4096;
const UNRELATED_BYTE: usize = 31 * PAGE_BYTES + 37;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::Text(text.to_owned()))
}

fn image(
    bytes: &[u8],
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, EditError> {
    Ok(NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        allowance,
        cancelled,
    )?)
}

fn with_groups<T>(
    case: &model::Case,
    reverse_groups: bool,
    reverse_rows: bool,
    action: impl FnOnce(&[FittedColumnUpdates<'_>]) -> T,
) -> T {
    let mut texts = case
        .texts
        .iter()
        .map(|update| {
            SlotScalarUpdate::new(update.row, text(update.expected), text(update.replacement))
        })
        .collect::<Vec<_>>();
    let mut fixed = case
        .fixed
        .iter()
        .map(|group| {
            group
                .updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.row,
                        update.expected.value(),
                        update.replacement.value(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if reverse_rows {
        texts.reverse();
        for updates in &mut fixed {
            updates.reverse();
        }
    }
    let mut groups = vec![FittedColumnUpdates::Variable {
        column: "Short_Text",
        updates: &texts,
    }];
    for (specification, updates) in case.fixed.iter().zip(&fixed) {
        groups.push(FittedColumnUpdates::Fixed {
            column: specification.name,
            updates,
        });
    }
    if reverse_groups {
        groups.reverse();
    }
    action(&groups)
}

fn run(
    case: &model::Case,
    reverse_groups: bool,
    reverse_rows: bool,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    with_groups(case, reverse_groups, reverse_rows, |groups| {
        image(&case.before, allowance, cancelled)?
            .preview_ace16_fitted_updates("Items", groups, allowance, cancelled)
    })
}

fn audit(case: &model::Case, observed: &[u8], changed_rows: usize) -> Result<(), EditError> {
    with_groups(case, false, false, |groups| {
        proof::prove(
            &image(&case.before, &mut budget(), &|| false)?,
            "Items",
            groups,
            Observed {
                bytes: observed,
                changed_rows,
            },
            &mut budget(),
            &|| false,
        )
    })
}

#[test]
fn joint_text_fixed_literal_images_match_all_group_and_row_orders() {
    let mut comparisons = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        audit(&case, expected, case.changed).unwrap();
        for reverse_groups in [false, true] {
            for reverse_rows in [false, true] {
                let result = run(&case, reverse_groups, reverse_rows, &mut budget(), &|| {
                    false
                })
                .unwrap();
                assert_eq!(result.candidate_bytes(), expected, "{recipe:?}");
                assert_eq!(result.changed_rows(), case.changed, "{recipe:?}");
                if case.changed == 0 {
                    assert_eq!(result.candidate_bytes(), case.before);
                }
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 72);
}

#[test]
fn joint_text_fixed_refusals_and_header_profile_are_not_order_dependent() {
    let mut refusals = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        if let Some(refusal) = case.refusal {
            let error = match refusal {
                model::Refusal::RowLayout => EditError::Unsupported(WriteBlocker::RowLayout),
                model::Refusal::EmptyText => EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::Field(FieldConstraintViolation::EmptyText),
                )),
                model::Refusal::Required => EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::Field(FieldConstraintViolation::Required),
                )),
                model::Refusal::Conflict => EditError::Conflict,
                model::Refusal::Column => EditError::Unsupported(WriteBlocker::Column),
                model::Refusal::IndexedColumn => {
                    EditError::Unsupported(WriteBlocker::IndexedColumn)
                }
            };
            for reverse in [false, true] {
                assert_eq!(
                    run(&case, reverse, reverse, &mut budget(), &|| false)
                        .err()
                        .as_ref(),
                    Some(&error),
                    "{recipe:?}"
                );
                refusals += 1;
            }
        }
        let older = model::source(5, recipe);
        assert!(matches!(
            run(&older, false, false, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    assert_eq!(refusals, 18);
}

#[test]
fn joint_text_fixed_exact_pools_each_one_below_and_final_cancel() {
    let case = model::source(6, model::Recipe::DisjointRows);
    let expected = case.expected.as_ref().unwrap();
    let mut measured = budget();
    let polls = Cell::new(0);
    let result = run(&case, false, false, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(result.candidate_bytes(), expected);
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    assert_eq!(
        run(
            &case,
            false,
            false,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false
        )
        .unwrap()
        .candidate_bytes(),
        expected
    );
    for position in 0..used.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case,
                false,
                false,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let last = polls.get();
    assert!(last > 0);
    polls.set(0);
    assert!(matches!(
        run(&case, false, false, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), last);
}

#[test]
fn joint_text_fixed_proof_rejects_payload_mask_sibling_footer_and_count_forgery() {
    let case = model::source(6, model::Recipe::BetaLarge);
    let expected = case.expected.as_ref().unwrap();
    audit(&case, expected, case.changed).unwrap();
    let frame = model::frame(expected, 0);
    // The field declaration supplies this original fixed offset independently
    // of any stage patch. Text variable movement cannot move the fixed prefix.
    let fixed = case.fixed[0].column.offset() + size_of::<u16>();
    let mut forgeries = Vec::new();
    for at in [
        frame.absolute + fixed,
        frame.absolute + frame.mask,
        frame.absolute + frame.offsets[1],
        frame.absolute + frame.count_at - size_of::<u16>(),
        UNRELATED_BYTE,
    ] {
        let mut altered = expected.clone();
        altered[at] ^= 1;
        assert_ne!(altered, *expected);
        forgeries.push(altered);
    }
    for observed in forgeries {
        assert_eq!(
            audit(&case, &observed, case.changed),
            Err(EditError::Preservation)
        );
    }
    assert_eq!(
        audit(&case, expected, case.changed + 1),
        Err(EditError::Preservation)
    );
    let mut truncated = expected.clone();
    truncated.pop();
    assert_eq!(
        audit(&case, &truncated, case.changed),
        Err(EditError::Preservation)
    );
    let mut alternative = model::source(6, model::Recipe::BetaLarge);
    alternative.fixed[0].updates[0].replacement = alternative.fixed[0].updates[0].expected;
    assert_eq!(
        audit(&alternative, expected, case.changed),
        Err(EditError::Preservation)
    );
}

#[test]
fn joint_text_fixed_proof_preserves_raw_and_inactive_noop_bytes() {
    for recipe in [
        model::Recipe::RawTextNoopFixed,
        model::Recipe::NullTextNoopFixed,
        model::Recipe::TextChangeInactiveDateNoop,
        model::Recipe::Int32TextNoopPadding,
    ] {
        let case = model::source(6, recipe);
        let expected = case.expected.as_ref().unwrap();
        audit(&case, expected, case.changed).unwrap();
        let frame = model::frame(expected, 0);
        let at = if recipe == model::Recipe::TextChangeInactiveDateNoop {
            frame.absolute + 2 + 16
        } else if recipe == model::Recipe::Int32TextNoopPadding {
            frame.absolute + 2 + 8 + size_of::<i32>()
        } else {
            let row = usize::from(recipe == model::Recipe::NullTextNoopFixed);
            let frame = model::frame(expected, row);
            frame.absolute + frame.offsets[0]
        };
        let mut normalized = expected.clone();
        normalized[at] ^= 1;
        assert_eq!(
            audit(&case, &normalized, case.changed),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn joint_text_fixed_proof_revalidates_final_rules_and_empty_target_metadata() {
    for (recipe, violation) in [
        (
            model::Recipe::TextRequired,
            FieldConstraintViolation::Required,
        ),
        (
            model::Recipe::NumericRequired,
            FieldConstraintViolation::Required,
        ),
        (
            model::Recipe::DisallowEmptyText,
            FieldConstraintViolation::EmptyText,
        ),
    ] {
        let case = model::source(6, recipe);
        assert!(case.expected.is_none());
        let original = image(&case.before, &mut budget(), &|| false).unwrap();
        source::capture(&original, "Items", &mut budget(), &|| false).unwrap();
        // This source/request-only constructor intentionally ignores field
        // rules. The physical record and requested selected values agree, so
        // rejection must come from the original qualified final constraints.
        let (self_consistent, changed) = model::reconstruct(&case.before, &case.texts, &case.fixed);
        assert_ne!(self_consistent, case.before);
        assert_eq!(
            audit(&case, &self_consistent, changed),
            Err(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(violation)
            )))
        );
    }
    let case = model::source(6, model::Recipe::EmptyIndexedFixed);
    assert!(case.fixed.iter().any(|group| group.updates.is_empty()));
    assert_eq!(
        audit(&case, &case.before, 0),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );

    let case = model::source(6, model::Recipe::BetaLarge);
    for (expected, value) in [
        (Value::Int32(1), Value::Text("beta".to_owned())),
        (Value::Text("alpha".to_owned()), Value::Int32(1)),
        (Value::Binary(vec![1]), Value::Null),
    ] {
        with_groups(&case, false, false, |groups| {
            let wrong = [SlotScalarUpdate::new(0, expected, value)];
            let replaced = groups
                .iter()
                .map(|group| match group {
                    FittedColumnUpdates::Variable { column, .. } => FittedColumnUpdates::Variable {
                        column,
                        updates: &wrong,
                    },
                    FittedColumnUpdates::Fixed { column, updates } => {
                        FittedColumnUpdates::Fixed { column, updates }
                    }
                })
                .collect::<Vec<_>>();
            assert!(matches!(
                image(&case.before, &mut budget(), &|| false)
                    .unwrap()
                    .preview_ace16_fitted_updates("Items", &replaced, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
            assert_eq!(
                proof::prove(
                    &image(&case.before, &mut budget(), &|| false).unwrap(),
                    "Items",
                    &replaced,
                    Observed {
                        bytes: &case.before,
                        changed_rows: 0
                    },
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Unsupported(WriteBlocker::Column))
            );
        });
    }
}
