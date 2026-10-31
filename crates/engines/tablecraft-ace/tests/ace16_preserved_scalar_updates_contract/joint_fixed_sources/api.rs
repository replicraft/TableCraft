//! Public native joint batches against prequalified independent complete images.
use super::{SCOPE, budget, model};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, ConstraintViolation, EditError, FixedColumnUpdates, FixedScalarUpdate,
    InspectError, NativeImage, WriteBlocker,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::FieldConstraintViolation;

fn ordered(case: &model::Case, reverse_groups: bool, reverse_rows: bool) -> Vec<model::Group> {
    let mut groups = case.groups.clone();
    if reverse_groups {
        groups.reverse();
    }
    if reverse_rows {
        for group in &mut groups {
            group.updates.reverse();
        }
    }
    groups
}

fn run(
    case: &model::Case,
    original_groups: &[model::Group],
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let assignments = original_groups
        .iter()
        .map(|group| {
            group
                .updates
                .iter()
                .map(|assignment| {
                    FixedScalarUpdate::new(
                        assignment.row,
                        assignment.expected.value(),
                        assignment.replacement.value(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let groups = original_groups
        .iter()
        .zip(&assignments)
        .map(|(group, assignments)| FixedColumnUpdates::new(group.table, group.name, assignments))
        .collect::<Vec<_>>();
    let image = NativeImage::capture(&mut Cursor::new(&case.before), SCOPE, allowance, cancelled)?;
    image.preview_ace16_batch_updates(&groups, allowance, cancelled)
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

#[test]
fn joint_fixed_batches_match_seventy_six_ordered_complete_images() {
    let mut comparisons = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        for reverse_groups in [false, true] {
            for reverse_rows in [false, true] {
                let groups = ordered(&case, reverse_groups, reverse_rows);
                let result = run(&case, &groups, &mut budget(), &|| false).unwrap();
                assert_image(result.before_bytes(), &case.before);
                assert_image(result.candidate_bytes(), expected);
                assert_eq!(result.changed_rows(), case.changed, "{recipe:?}");
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 76);
}

#[test]
fn joint_fixed_refusals_retain_exact_typed_errors_in_every_order() {
    let mut refused = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(refusal) = case.refusal else {
            continue;
        };
        assert!(case.expected.is_none());
        for reverse_groups in [false, true] {
            for reverse_rows in [false, true] {
                let groups = ordered(&case, reverse_groups, reverse_rows);
                let error = run(&case, &groups, &mut budget(), &|| false).err().unwrap();
                let expected = match refusal {
                    model::Refusal::Conflict => EditError::Conflict,
                    model::Refusal::Required => EditError::Inspect(InspectError::Constraint(
                        ConstraintViolation::Field(FieldConstraintViolation::Required),
                    )),
                    model::Refusal::IndexedColumn => {
                        EditError::Unsupported(WriteBlocker::IndexedColumn)
                    }
                    model::Refusal::Column => EditError::Unsupported(WriteBlocker::Column),
                };
                assert_eq!(error, expected, "{recipe:?}");
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 32);

    for recipe in model::RECIPES {
        let case = model::source(5, recipe);
        assert!(matches!(
            run(&case, &case.groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
}

#[test]
fn joint_fixed_checks_alias_staleness_same_width_kinds_and_empty_metadata() {
    let case = model::source(6, model::Recipe::DateAliases);
    let mut stale = case.groups.clone();
    stale[0].updates[0].expected = model::Cell::Date(0x8000_0000_0000_0000);
    // The original +0 and supplied -0 are temporally equal, not exact expected
    // carriers. No scalar comparison or DATE normalization may hide staleness.
    assert!(matches!(
        run(&case, &stale, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));

    let case = model::source(6, model::Recipe::ThreePayloads);
    let mut wrong_kind = case.groups.clone();
    let currency = wrong_kind
        .iter_mut()
        .find(|group| group.column == model::Column::Currency)
        .unwrap();
    // Exact same eight payload bytes with an Int64 carrier still cannot be
    // supplied as Currency. This is neither a coercion nor a stale-value test.
    currency.updates[0].expected = model::Cell::Int64(12345);
    currency.updates[0].replacement = model::Cell::Int64(-10001);
    assert!(matches!(
        run(&case, &wrong_kind, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));

    let case = model::source(6, model::Recipe::EntirelyEmpty);
    let mut unknown = case.groups.clone();
    unknown[0].name = "Missing";
    assert!(matches!(
        run(&case, &unknown, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn joint_fixed_cumulative_exact_pools_and_final_cancellation_are_atomic() {
    let case = model::source(6, model::Recipe::OverlapRows);
    let expected = case.expected.as_ref().unwrap();
    let mut measured = budget();
    let polls = Cell::new(0);
    let candidate = run(&case, &case.groups, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_image(candidate.candidate_bytes(), expected);
    assert_eq!(candidate.changed_rows(), 3);
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let exact = run(
        &case,
        &case.groups,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert_image(exact.candidate_bytes(), expected);
    for position in 0..used.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case,
                &case.groups,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let last = polls.get();
    assert!(last > 0);
    polls.set(0);
    assert!(matches!(
        run(&case, &case.groups, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), last);
}
