//! Public native fitting updates against the unchanged rich SOURCE expectations.
use super::{SCOPE, budget, model};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, ConstraintViolation, EditError, FittedColumnUpdates, FixedScalarUpdate,
    InspectError, NativeImage, PropertyGroupKind, SlotScalarUpdate, WriteBlocker,
    inspect_table_properties, read_table_projection,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::FieldConstraintViolation;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_owned()))
}

fn request(case: &model::Case) -> SlotScalarUpdate {
    SlotScalarUpdate::new(
        case.framing.row,
        text(case.original),
        text(case.replacement),
    )
}

fn run(
    bytes: &[u8],
    requests: &[SlotScalarUpdate],
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_preserved_text_updates(
        "Items",
        "Short_Text",
        requests,
        allowance,
        cancelled,
    )
}

#[test]
fn preserved_short_text_all_eight_complete_images_and_header_five_refusals() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            if recipe == model::Recipe::Overflow {
                continue;
            }
            let requests = [request(&case)];
            let result = run(&case.before, &requests, &mut budget(), &|| false);
            if code == 5 {
                assert!(matches!(
                    result,
                    Err(EditError::Unsupported(WriteBlocker::Column))
                ));
                refused += 1;
                continue;
            }
            let candidate = result.unwrap();
            assert!(candidate.before_bytes() == case.before);
            assert!(candidate.candidate_bytes() == case.expected.as_ref().unwrap());
            assert_eq!(
                candidate.changed_rows(),
                usize::from(case.original != case.replacement)
            );
            assert_eq!(candidate.candidate_bytes().len(), case.before.len());
            admitted += 1;
        }
    }
    assert_eq!((admitted, refused), (8, 8));
}

#[test]
fn preserved_short_text_empty_and_raw_text_and_null_noops_preserve_every_byte() {
    for recipe in [model::Recipe::Noop, model::Recipe::NullNoop] {
        let case = model::source(6, recipe);
        for requests in [Vec::new(), vec![request(&case)]] {
            let candidate = run(&case.before, &requests, &mut budget(), &|| false).unwrap();
            assert_eq!(candidate.changed_rows(), 0);
            assert!(candidate.before_bytes() == case.before);
            assert!(candidate.candidate_bytes() == case.before);
        }
    }
    let case = model::source(5, model::Recipe::Noop);
    assert!(matches!(
        run(&case.before, &[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn preserved_short_text_stale_carriers_duplicates_and_invalid_rows_return_no_candidate() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    for expected in [Value::Null, Value::Text("stale".to_owned())] {
        assert!(matches!(
            run(
                &case.before,
                &[SlotScalarUpdate::new(
                    0,
                    expected,
                    Value::Text("beta".to_owned())
                )],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Conflict)
        ));
    }
    for (expected, replacement) in [
        (Value::Int32(10), Value::Text("beta".to_owned())),
        (Value::Text("alpha".to_owned()), Value::Binary(vec![1, 2])),
        (Value::Text("alpha".to_owned()), Value::Boolean(true)),
    ] {
        assert!(matches!(
            run(
                &case.before,
                &[SlotScalarUpdate::new(0, expected, replacement)],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    let duplicates = [request(&case), request(&case)];
    assert!(matches!(
        run(&case.before, &duplicates, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
    for row in [3, usize::MAX] {
        let requests = [SlotScalarUpdate::new(
            row,
            Value::Text("alpha".to_owned()),
            Value::Text("beta".to_owned()),
        )];
        assert!(matches!(
            run(&case.before, &requests, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
    }
    let null = model::source(6, model::Recipe::NullToValue);
    assert!(matches!(
        run(
            &null.before,
            &[SlotScalarUpdate::new(
                1,
                Value::Text("ignored".to_owned()),
                Value::Text("é".to_owned())
            )],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Conflict)
    ));
}

#[test]
fn preserved_short_text_retains_target_capacity_refusals_and_joint_fitting_profile() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    for (column, expected) in [("ID", 1), ("Number", 1), ("Calculated", 2)] {
        let updates = [SlotScalarUpdate::new(
            0,
            Value::Int32(expected),
            Value::Int32(99),
        )];
        assert!(matches!(
            capture(&case.before).preview_ace16_preserved_text_updates(
                "Items",
                column,
                &updates,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    for column in [
        "Long_Text",
        "Hyperlink_Field",
        "OLE_Object_Field",
        "missing",
    ] {
        assert!(matches!(
            capture(&case.before).preview_ace16_preserved_text_updates(
                "Items",
                column,
                &[],
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    // Joint fitting now composes the independent Text golden with the literal
    // Boolean value bit. Every other byte remains part of the comparison.
    let mut joint_expected = case.expected.clone().unwrap();
    joint_expected[13 * 4096 - 1] |= 0x04;
    let text_updates = [request(&case)];
    let fixed_updates = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    for reverse in [false, true] {
        let mut groups = [
            FittedColumnUpdates::Variable {
                column: "Short_Text",
                updates: &text_updates,
            },
            FittedColumnUpdates::Fixed {
                column: "Yes_Field",
                updates: &fixed_updates,
            },
        ];
        if reverse {
            groups.reverse();
        }
        let candidate = capture(&case.before)
            .preview_ace16_fitted_updates("Items", &groups, &mut budget(), &|| false)
            .unwrap();
        assert!(candidate.candidate_bytes() == joint_expected);
        assert_eq!(candidate.changed_rows(), 1);
    }
    let overflow = model::source(6, model::Recipe::Overflow);
    assert!(overflow.expected.is_none());
    assert!(matches!(
        run(
            &overflow.before,
            &[request(&overflow)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn preserved_short_text_exact_operation_pools_and_final_cancellation() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    let requests = [request(&case)];
    let mut measured = budget();
    let polls = Cell::new(0);
    let candidate = run(&case.before, &requests, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert!(candidate.candidate_bytes() == case.expected.as_ref().unwrap());
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    assert!(
        run(
            &case.before,
            &requests,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false
        )
        .is_ok()
    );
    for position in 0..resources.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case.before,
                &requests,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let final_poll = polls.get();
    assert!(final_poll > 0);
    polls.set(0);
    assert!(matches!(
        run(&case.before, &requests, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), final_poll);
}

#[test]
fn preserved_short_text_healthy_zero_length_prohibition_returns_field_violation() {
    let case = model::source(6, model::Recipe::Empty);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let (before, patch) =
        super::zero_length_source::disallow_empty(&case.before, properties.raw_bytes());
    assert_eq!((case.before[patch], before[patch]), (1, 0));
    let properties = inspect_table_properties(
        &mut Cursor::new(&before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        properties
            .group(PropertyGroupKind::Column, "Short_Text")
            .unwrap()
            .get("AllowZeroLength")
            .unwrap()
            .boolean(),
        Ok(false)
    );
    let rows = read_table_projection(
        &mut Cursor::new(&before),
        SCOPE,
        "Items",
        &["Short_Text"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        rows.rows(),
        [
            vec![Value::Text("alpha".to_owned())],
            vec![Value::Null],
            vec![Value::Text("omega".to_owned())]
        ]
    );
    assert!(matches!(
        run(&before, &[request(&case)], &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::Field(FieldConstraintViolation::EmptyText)
        )))
    ));
    assert_eq!(&before[..patch], &case.before[..patch]);
    assert_eq!(&before[patch + 1..], &case.before[patch + 1..]);
}
