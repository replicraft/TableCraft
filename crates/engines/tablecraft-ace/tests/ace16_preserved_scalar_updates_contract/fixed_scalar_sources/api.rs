//! Existing fixed and batch APIs against independently frozen whole images.
use super::{SCOPE, budget, model};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, ConstraintViolation, EditError, FixedColumnUpdates, FixedScalarUpdate,
    InspectError, NativeImage, WriteBlocker,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::FieldConstraintViolation;

fn requests(case: &model::Case) -> Vec<FixedScalarUpdate> {
    (0..case.requested_rows)
        .map(|row| {
            FixedScalarUpdate::new(
                row,
                case.target.value(case.initial[row]),
                case.target.value(case.replacement[row]),
            )
        })
        .collect()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "Complete image length");
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (actual, expected))| actual != expected)
        .take(8)
        .map(|(offset, (actual, expected))| (offset, *actual, *expected))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "First byte differences: {differences:?}"
    );
}

fn run(
    case: &model::Case,
    updates: &[FixedScalarUpdate],
    batch: bool,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), SCOPE, allowance, cancelled)?;
    if batch {
        image.preview_ace16_batch_updates(
            &[FixedColumnUpdates::new(
                "Items",
                case.target.name(),
                updates,
            )],
            allowance,
            cancelled,
        )
    } else {
        image.preview_ace16_fixed_updates(
            "Items",
            case.target.name(),
            updates,
            allowance,
            cancelled,
        )
    }
}

#[test]
fn fixed_and_batch_match_seventy_complete_scalar_images() {
    let mut comparisons = 0;
    for (target, recipe) in model::cases() {
        let case = model::source(6, target, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        let updates = requests(&case);
        for batch in [false, true] {
            let result = run(&case, &updates, batch, &mut budget(), &|| false).unwrap();
            assert_image(result.before_bytes(), &case.before);
            assert_image(result.candidate_bytes(), expected);
            assert_eq!(result.changed_rows(), case.changed, "{target:?} {recipe:?}");
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 70);
}

#[test]
fn required_fields_and_unqualified_extended_date_siblings_refuse() {
    for target in model::TARGETS {
        let case = model::source(6, target, model::Recipe::RequiredRefusal);
        for batch in [false, true] {
            assert!(matches!(
                run(&case, &requests(&case), batch, &mut budget(), &|| false),
                Err(EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::Field(FieldConstraintViolation::Required)
                )))
            ));
        }
    }

    for (target, recipe) in model::cases() {
        let case = model::source(5, target, recipe);
        for batch in [false, true] {
            assert!(matches!(
                run(&case, &requests(&case), batch, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
    }
}

#[test]
fn empty_requests_resolve_actual_metadata_and_preserve_the_image() {
    for target in model::TARGETS {
        let case = model::source(6, target, model::Recipe::NullNoop);
        for batch in [false, true] {
            let result = run(&case, &[], batch, &mut budget(), &|| false).unwrap();
            assert_image(result.candidate_bytes(), &case.before);
            assert_eq!(result.changed_rows(), 0);
        }
    }
}

#[test]
fn reversed_requests_retain_logical_rows_and_mixed_groups_refuse() {
    for target in model::TARGETS {
        let case = model::source(6, target, model::Recipe::MixedRows);
        let mut updates = requests(&case);
        updates.reverse();
        for batch in [false, true] {
            let result = run(&case, &updates, batch, &mut budget(), &|| false).unwrap();
            assert_image(result.candidate_bytes(), case.expected.as_ref().unwrap());
            assert_eq!(result.changed_rows(), case.changed);
        }

        let boolean = [FixedScalarUpdate::new(
            0,
            Value::Boolean(false),
            Value::Boolean(true),
        )];
        for reverse in [false, true] {
            let mut groups = [
                FixedColumnUpdates::new("Items", target.name(), &updates),
                FixedColumnUpdates::new("Items", "Yes_Field", &boolean),
            ];
            if reverse {
                groups.reverse();
            }
            let mut allowance = budget();
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut allowance,
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                image.preview_ace16_batch_updates(&groups, &mut allowance, &|| false),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
    }
}

#[test]
fn stale_presence_wrong_carriers_and_duplicate_ordinals_refuse() {
    for target in model::TARGETS {
        let absent = model::source(6, target, model::Recipe::NullToValue);
        let present = model::source(6, target, model::Recipe::ValueToNull);
        for case in [&absent, &present] {
            let stale = if case.initial[0].is_some() {
                Value::Null
            } else {
                target.value(case.replacement[0])
            };
            let updates = [FixedScalarUpdate::new(0, stale, Value::Null)];
            for batch in [false, true] {
                assert!(matches!(
                    run(case, &updates, batch, &mut budget(), &|| false),
                    Err(EditError::Conflict)
                ));
            }
        }

        for updates in [
            vec![FixedScalarUpdate::new(3, Value::Null, Value::Null)],
            vec![
                FixedScalarUpdate::new(0, Value::Null, Value::Null),
                FixedScalarUpdate::new(0, Value::Null, Value::Null),
            ],
        ] {
            for batch in [false, true] {
                assert!(matches!(
                    run(&absent, &updates, batch, &mut budget(), &|| false),
                    Err(EditError::Conflict)
                ));
            }
        }

        for other in model::TARGETS {
            if other.native() == target.native() {
                continue;
            }
            let other_case = model::source(6, other, model::Recipe::Change);
            let updates = [FixedScalarUpdate::new(
                0,
                Value::Null,
                other.value(other_case.replacement[0]),
            )];
            for batch in [false, true] {
                assert!(matches!(
                    run(&absent, &updates, batch, &mut budget(), &|| false),
                    Err(EditError::Unsupported(WriteBlocker::Column))
                ));
            }
        }
    }
}

#[test]
fn cumulative_exact_pools_and_final_cancellation_cover_each_carrier() {
    for target in model::TARGETS {
        let case = model::source(6, target, model::Recipe::MixedRows);
        let updates = requests(&case);
        let mut measured = budget();
        let polls = Cell::new(0);
        let candidate = run(&case, &updates, true, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_image(candidate.candidate_bytes(), case.expected.as_ref().unwrap());

        let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
            .map(|resource| budget().remaining(resource) - measured.remaining(resource));
        assert!(used.iter().all(|amount| *amount > 0));
        let exact = run(
            &case,
            &updates,
            true,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false,
        )
        .unwrap();
        assert_image(exact.candidate_bytes(), case.expected.as_ref().unwrap());

        for position in 0..used.len() {
            let mut limits = used;
            limits[position] -= 1;
            assert!(matches!(
                run(
                    &case,
                    &updates,
                    true,
                    &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                    &|| false,
                ),
                Err(EditError::Inspect(InspectError::Limit(_)))
            ));
        }

        let last = polls.get();
        polls.set(0);
        assert!(matches!(
            run(&case, &updates, true, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == last
            }),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(polls.get(), last);
    }
}
