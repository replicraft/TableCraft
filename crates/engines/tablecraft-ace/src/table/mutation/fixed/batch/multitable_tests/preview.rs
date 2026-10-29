//! API comparisons and atomic refusals against the independent two-table owner.
use super::{budget, capture, model, qualify_table};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexConstraintViolation, InspectError,
    WriteBlocker, table::mutation::insert::tests::multiple_indexes::model as frames,
};
use model::{Case, Mode, PAGE, Settings};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_testkit::access as raw;

struct Updates {
    left: Vec<FixedScalarUpdate>,
    right: Vec<FixedScalarUpdate>,
}

fn updates(case: &Case, reverse: bool) -> [Updates; 2] {
    case.tables.each_ref().map(|table| {
        let count = if table.fitted { 2 } else { 500 };
        let component = |right| {
            let mut result = table
                .original
                .iter()
                .zip(&table.final_rows)
                .take(count)
                .enumerate()
                .map(|(ordinal, (old, new))| {
                    let (expected, value) = if right {
                        (old.right, new.right)
                    } else {
                        (old.left, new.left)
                    };
                    FixedScalarUpdate::new(
                        ordinal,
                        expected.map_or(Value::Null, Value::Int32),
                        value.map_or(Value::Null, Value::Int32),
                    )
                })
                .collect::<Vec<_>>();
            if reverse {
                result.reverse();
            }
            result
        };
        Updates {
            left: component(false),
            right: component(true),
        }
    })
}

fn groups<'a>(
    case: &Case,
    updates: &'a [Updates; 2],
    reverse: bool,
) -> Vec<FixedColumnUpdates<'a>> {
    let mut result = Vec::new();
    for (table, updates) in case.tables.iter().zip(updates) {
        result.push(FixedColumnUpdates::new(table.name, "Group", &updates.left));
        result.push(FixedColumnUpdates::new(table.name, "Score", &updates.right));
    }
    if reverse {
        result.reverse();
    }
    result
}

fn assert_image(actual: &[u8], expected: &[u8], label: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{label}: complete image length"
    );
    let mut count = 0;
    let mut first = None;
    for (offset, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        if actual != expected {
            count += 1;
            first.get_or_insert((offset, actual, expected));
        }
    }
    assert_eq!(count, 0, "{label}: first differing byte {first:?}");
}

fn example(unique: bool) -> Case {
    model::source(Settings {
        code: 6,
        descending: true,
        unique,
        forwarded: false,
        mode: Mode::Full,
    })
}

#[test]
fn two_table_allocation_matches_forty_eight_complete_images_and_reversed_requests() {
    for settings in model::settings() {
        let case = model::source(settings);
        for reverse in [false, true] {
            let updates = updates(&case, reverse);
            let groups = groups(&case, &updates, reverse);
            let label = format!("{settings:?} reversed={reverse}");
            let result = capture(&case.before)
                .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{label}: {error:?}"));
            assert_image(result.before_bytes(), &case.before, &label);
            assert_image(result.candidate_bytes(), &case.expected, &label);
            assert_eq!(
                result.changed_rows(),
                if case.tables.iter().any(|table| table.fitted) {
                    502
                } else {
                    1000
                }
            );
        }
    }
}

#[test]
fn late_second_table_stale_or_unique_failure_never_returns_the_first_table_candidate() {
    let case = example(false);
    let mut assignments = updates(&case, false);
    assignments[1].right[499] = FixedScalarUpdate::new(
        499,
        Value::Int32(77),
        case.tables[1].final_rows[499]
            .right
            .map_or(Value::Null, Value::Int32),
    );
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(
                &groups(&case, &assignments, false),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Conflict)
    );

    let case = example(true);
    let mut assignments = updates(&case, false);
    for ordinal in 0..2 {
        assignments[1].left[ordinal] =
            FixedScalarUpdate::new(ordinal, Value::Null, Value::Int32(42));
    }
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(
                &groups(&case, &assignments, false),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    );
}

fn overflow_source() -> Case {
    let mut case = example(false);
    raw::put32(&mut case.before, 20 * PAGE + 79, u32::MAX);
    case
}

fn unreachable_source() -> Case {
    let mut case = example(false);
    frames::map_page(
        &mut case.before,
        25,
        vec![frames::map([23, 29]), frames::map([24])],
    );
    case
}

#[test]
fn qualify_multitable_overflow_and_unreachable_owned_sources_before_preview() {
    let overflow = overflow_source();
    for table in &overflow.tables {
        qualify_table(
            &overflow.before,
            table,
            false,
            if table.definition == 20 {
                [700, u32::MAX]
            } else {
                [700, 800]
            },
        );
    }
    let unreachable = unreachable_source();
    for table in &unreachable.tables {
        qualify_table(&unreachable.before, table, false, [700, 800]);
    }
}

#[test]
fn late_counter_exhaustion_and_owned_unreachable_second_tree_refuse_atomically() {
    let case = overflow_source();
    let assignments = updates(&case, false);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(
                &groups(&case, &assignments, false),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    let case = unreachable_source();
    let assignments = updates(&case, false);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(
                &groups(&case, &assignments, false),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
}

#[test]
fn shared_multiowner_operation_pays_every_pool_and_cancels_at_the_last_poll() {
    let case = example(false);
    let assignments = updates(&case, false);
    let groups = groups(&case, &assignments, false);
    let mut measured = budget();
    let polls = Cell::new(0);
    let healthy = capture(&case.before)
        .preview_ace16_split_batch_updates(&groups, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_image(
        healthy.candidate_bytes(),
        &case.expected,
        "healthy shared ledger",
    );
    let terminal = polls.get();
    assert!(terminal > 1);
    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        let consumed = budget().remaining(resource) - measured.remaining(resource);
        assert!(consumed > 0);
        let mut limits = [512 * 1024 * 1024, 100_000, 500_000_000];
        limits[position] = consumed - 1;
        let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
        assert!(
            matches!(capture(&case.before).preview_ace16_split_batch_updates(&groups, &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }
    polls.set(0);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            })
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}
