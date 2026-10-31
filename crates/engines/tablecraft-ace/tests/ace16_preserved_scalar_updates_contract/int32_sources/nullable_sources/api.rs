//! Public fixed/batch acceptance against the unchanged nullable source owner.
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

fn value(number: Option<i32>) -> Value {
    match number {
        Some(number) => Value::Int32(number),
        None => Value::Null,
    }
}

fn requests(case: &model::Case) -> Vec<FixedScalarUpdate> {
    (0..case.requested_rows)
        .map(|row| {
            FixedScalarUpdate::new(row, value(case.initial[row]), value(case.replacement[row]))
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
    bytes: &[u8],
    updates: &[FixedScalarUpdate],
    batch: bool,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image = NativeImage::capture(&mut Cursor::new(bytes), SCOPE, allowance, cancelled)?;
    if batch {
        image.preview_ace16_batch_updates(
            &[FixedColumnUpdates::new("Items", "Large_Number", updates)],
            allowance,
            cancelled,
        )
    } else {
        image.preview_ace16_fixed_updates("Items", "Large_Number", updates, allowance, cancelled)
    }
}

#[test]
fn nullable_int32_fixed_and_batch_match_six_complete_images() {
    let mut comparisons = 0;
    for name in model::CASES {
        let case = model::source(name);
        let Some(expected) = &case.expected else {
            assert!(case.required);
            continue;
        };
        let updates = requests(&case);
        let changed = case
            .initial
            .iter()
            .zip(case.replacement)
            .filter(|(before, after)| **before != *after)
            .count();
        for batch in [false, true] {
            let result = run(&case.before, &updates, batch, &mut budget(), &|| false).unwrap();
            assert_image(result.before_bytes(), &case.before);
            assert_image(result.candidate_bytes(), expected);
            assert_eq!(result.changed_rows(), changed);
            if changed == 0 {
                assert_image(result.candidate_bytes(), &case.before);
            }
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 12);
}

#[test]
fn nullable_int32_required_and_stale_carriers_are_typed_refusals() {
    let required = model::source("required_refusal");
    let required_updates = requests(&required);
    for batch in [false, true] {
        assert!(matches!(
            run(
                &required.before,
                &required_updates,
                batch,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(FieldConstraintViolation::Required)
            )))
        ));
    }

    let absent = model::source("null_to_value");
    let present = model::source("value_to_null");
    for (bytes, expected, replacement) in [
        (&absent.before, Value::Int32(1000), Value::Int32(101)),
        (&absent.before, Value::Int32(0), Value::Null),
        (&present.before, Value::Null, Value::Int32(101)),
        (&present.before, Value::Int32(999), Value::Null),
    ] {
        let updates = [FixedScalarUpdate::new(0, expected, replacement)];
        for batch in [false, true] {
            assert!(matches!(
                run(bytes, &updates, batch, &mut budget(), &|| false),
                Err(EditError::Conflict)
            ));
        }
    }

    for (expected, replacement) in [
        (Value::Null, Value::Text("101".into())),
        (Value::Null, Value::Int64(101)),
    ] {
        let updates = [FixedScalarUpdate::new(0, expected, replacement)];
        for batch in [false, true] {
            assert!(matches!(
                run(&absent.before, &updates, batch, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
    }
}

#[test]
fn nullable_int32_duplicate_ordinals_refuse_and_joint_boolean_matches() {
    let case = model::source("null_to_value");
    for updates in [
        vec![FixedScalarUpdate::new(3, Value::Null, Value::Int32(101))],
        vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(101)),
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(-101)),
        ],
    ] {
        for batch in [false, true] {
            assert!(matches!(
                run(&case.before, &updates, batch, &mut budget(), &|| false),
                Err(EditError::Conflict)
            ));
        }
    }

    let numeric = requests(&case);
    let boolean = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    let yes_mask = 13 * 4096 - 1;
    let yes_bit = 1_u8 << 2;
    assert_eq!(case.before[yes_mask] & yes_bit, 0);
    let mut joint_expected = case.expected.as_ref().unwrap().clone();
    joint_expected[yes_mask] |= yes_bit;
    for reverse in [false, true] {
        let mut groups = [
            FixedColumnUpdates::new("Items", "Large_Number", &numeric),
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
        let result = image
            .preview_ace16_batch_updates(&groups, &mut allowance, &|| false)
            .unwrap();
        assert_eq!(result.before_bytes(), case.before);
        assert_eq!(result.candidate_bytes(), joint_expected);
        assert_eq!(result.changed_rows(), 1);
    }
}

#[test]
fn nullable_int32_empty_requests_qualify_metadata_and_preserve_every_byte() {
    let case = model::source("null_to_value");
    for batch in [false, true] {
        let result = run(&case.before, &[], batch, &mut budget(), &|| false).unwrap();
        assert_image(result.candidate_bytes(), &case.before);
        assert_eq!(result.changed_rows(), 0);
    }
}

#[test]
fn nullable_int32_cumulative_exact_pools_and_final_cancellation_refuse_without_candidate() {
    let case = model::source("mixed_rows");
    let updates = requests(&case);
    let mut measured = budget();
    let polls = Cell::new(0);
    let candidate = run(&case.before, &updates, true, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_image(candidate.candidate_bytes(), case.expected.as_ref().unwrap());
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let exact = run(
        &case.before,
        &updates,
        true,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert_image(exact.candidate_bytes(), case.expected.as_ref().unwrap());

    for position in 0..resources.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case.before,
                &updates,
                true,
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
        run(&case.before, &updates, true, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), last);
}
