//! Actual existing fixed/batch APIs against unchanged independent source pairs.
use super::{SCOPE, budget, int32_sources::model};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage,
    WriteBlocker,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn run(
    bytes: &[u8],
    value: i32,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_fixed_updates(
        "Items",
        "Large_Number",
        &[FixedScalarUpdate::new(
            0,
            Value::Int32(1000),
            Value::Int32(value),
        )],
        allowance,
        cancelled,
    )
}

#[test]
fn preserved_int32_four_positive_images_and_four_profile_refusals() {
    let mut admitted = 0;
    let mut refused = 0;
    for (code, value) in model::CASES {
        let case = model::source(code, value);
        let result = run(&case.before, value, &mut budget(), &|| false);
        if code == 5 {
            assert!(matches!(
                result,
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
            refused += 1;
            continue;
        }
        let candidate = result.unwrap();
        assert_eq!(candidate.before_bytes(), case.before);
        assert_eq!(candidate.candidate_bytes(), case.expected);
        assert_eq!(candidate.changed_rows(), 1);
        let requests = [FixedScalarUpdate::new(
            0,
            Value::Int32(1000),
            Value::Int32(value),
        )];
        let batch = capture(&case.before)
            .preview_ace16_batch_updates(
                &[FixedColumnUpdates::new("Items", "Large_Number", &requests)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(batch.candidate_bytes(), case.expected);
        assert_eq!(batch.changed_rows(), 1);
        assert_eq!(
            &candidate.candidate_bytes()[case.patch + 4..case.patch + 8],
            &[0; 4]
        );
        admitted += 1;
    }
    assert_eq!((admitted, refused), (4, 4));
}

#[test]
fn preserved_int32_empty_and_equal_operations_retain_original_bytes() {
    let case = model::source(6, -101);
    for requests in [
        Vec::new(),
        vec![FixedScalarUpdate::new(
            0,
            Value::Int32(1000),
            Value::Int32(1000),
        )],
    ] {
        let result = capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Large_Number", &requests, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(result.candidate_bytes(), case.before);
        assert_eq!(result.changed_rows(), 0);
    }
    let refused = model::source(5, -101);
    assert!(matches!(
        capture(&refused.before).preview_ace16_fixed_updates(
            "Items",
            "Large_Number",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn preserved_int32_stale_null_wrong_carriers_and_mixed_groups_refuse() {
    let case = model::source(6, -101);
    for (expected, value) in [
        (Value::Int32(999), Value::Int32(-101)),
        (Value::Null, Value::Int32(-101)),
        (Value::Boolean(false), Value::Boolean(true)),
        (Value::Int32(1000), Value::Int64(-101)),
    ] {
        assert!(
            capture(&case.before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Large_Number",
                    &[FixedScalarUpdate::new(0, expected, value)],
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    for requests in [
        vec![FixedScalarUpdate::new(
            3,
            Value::Int32(1000),
            Value::Int32(-101),
        )],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(1000), Value::Int32(-101)),
            FixedScalarUpdate::new(0, Value::Int32(1000), Value::Int32(101)),
        ],
    ] {
        assert!(
            capture(&case.before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Large_Number",
                    &requests,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    let numeric = [FixedScalarUpdate::new(
        0,
        Value::Int32(1000),
        Value::Int32(-101),
    )];
    let boolean = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    for reverse in [false, true] {
        let mut groups = [
            FixedColumnUpdates::new("Items", "Large_Number", &numeric),
            FixedColumnUpdates::new("Items", "Yes_Field", &boolean),
        ];
        if reverse {
            groups.reverse();
        }
        assert!(
            capture(&case.before)
                .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
                .is_err()
        );
    }
    for name in ["Number", "ID", "Calculated"] {
        assert!(
            capture(&case.before)
                .preview_ace16_fixed_updates(
                    "Items",
                    name,
                    &[FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(4))],
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
}

#[test]
fn preserved_int32_exact_pools_and_final_cancellation_return_no_candidate() {
    let case = model::source(6, -101);
    let mut measured = budget();
    let polls = Cell::new(0);
    let candidate = run(&case.before, -101, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(candidate.candidate_bytes(), case.expected);
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used = pools.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    assert!(
        run(
            &case.before,
            -101,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false
        )
        .is_ok()
    );
    for position in 0..3 {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case.before,
                -101,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let last = polls.get();
    polls.set(0);
    assert!(matches!(
        run(&case.before, -101, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), last);
}
