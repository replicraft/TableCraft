//! Public growth requests against the independently admitted complete images.
use super::super::{SCOPE, budget, short_text_sources::zero_length_source};
use super::model;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, ConstraintViolation, EditError, InspectError, NativeImage,
    PropertyGroupKind, SlotScalarUpdate, WriteBlocker, inspect_table_properties,
    read_table_projection,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::FieldConstraintViolation;

fn text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_owned()))
}

fn requests(case: &model::Case) -> Vec<SlotScalarUpdate> {
    case.requests
        .iter()
        .map(|request| {
            SlotScalarUpdate::new(
                request.row,
                text(request.expected),
                text(request.replacement),
            )
        })
        .collect()
}

fn run(
    before: &[u8],
    updates: &[SlotScalarUpdate],
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image =
        NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap();
    image.preview_ace16_preserved_text_growth("Items", "Short_Text", updates, allowance, cancelled)
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (actual, expected))| actual != expected)
        .take(8)
        .map(|(at, (actual, expected))| (at, *actual, *expected))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "First image differences: {differences:?}"
    );
}

#[test]
fn preserved_text_growth_matches_all_eight_independent_complete_images() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        let result = run(&case.before, &requests(&case), &mut budget(), &|| false).unwrap();
        assert_image(result.before_bytes(), &case.before);
        assert_image(result.candidate_bytes(), &case.expected);
        assert_eq!(result.changed_rows(), case.changed, "{shape:?}");
    }
}

#[test]
fn preserved_text_growth_empty_and_noop_requests_still_qualify_the_source() {
    let case = model::source(model::Shape::Noops);
    for updates in [Vec::new(), requests(&case)] {
        let result = run(&case.before, &updates, &mut budget(), &|| false).unwrap();
        assert_image(result.candidate_bytes(), &case.before);
        assert_eq!(result.changed_rows(), 0);
    }
    let mut header_five = case.before;
    header_five[20] = 5;
    assert!(matches!(
        run(&header_five, &[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn preserved_text_growth_rejects_stale_carriers_duplicates_and_invalid_ordinals() {
    let case = model::source(model::Shape::OriginalPage);
    for updates in [
        vec![SlotScalarUpdate::new(
            0,
            text(Some("stale")),
            text(Some("snow")),
        )],
        vec![SlotScalarUpdate::new(
            0,
            Value::Int32(1),
            text(Some("snow")),
        )],
        vec![SlotScalarUpdate::new(
            0,
            text(Some("alpha")),
            Value::Int32(1),
        )],
        vec![SlotScalarUpdate::new(3, Value::Null, text(Some("snow")))],
        vec![
            SlotScalarUpdate::new(0, text(Some("alpha")), text(Some("snow"))),
            SlotScalarUpdate::new(0, text(Some("alpha")), text(Some("snow"))),
        ],
    ] {
        assert!(run(&case.before, &updates, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn preserved_text_growth_exact_pools_and_final_cancellation_are_cumulative() {
    let case = model::source(model::Shape::EndOfFile);
    let updates = requests(&case);
    let mut allowance = budget();
    let polls = Cell::new(0_usize);
    let result = run(&case.before, &updates, &mut allowance, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_image(result.candidate_bytes(), &case.expected);

    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|resource| budget().remaining(resource) - allowance.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    let result = run(&case.before, &updates, &mut exact, &|| false).unwrap();
    assert_image(result.candidate_bytes(), &case.expected);
    for resource in 0..used.len() {
        let mut limited = used;
        limited[resource] -= 1;
        let mut allowance = ResourceBudget::new(limited[0], limited[1], limited[2]);
        assert!(matches!(
            run(&case.before, &updates, &mut allowance, &|| false),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }

    let last = polls.get();
    let polls = Cell::new(0_usize);
    assert!(matches!(
        run(&case.before, &updates, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn preserved_text_growth_final_source_rejects_empty_text_after_an_earlier_growth() {
    let case = model::source(model::Shape::OriginalPage);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let (before, patch) = zero_length_source::disallow_empty(&case.before, properties.raw_bytes());
    // This is the same one-byte derivative already qualified by the existing
    // preserved Short Text SOURCE gate, not a mutation-derived source image.
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
            vec![text(Some("alpha"))],
            vec![Value::Null],
            vec![text(Some("omega"))]
        ]
    );
    for reverse in [false, true] {
        let mut updates = [
            SlotScalarUpdate::new(0, text(Some("alpha")), text(Some("雪雪雪雪雪雪"))),
            SlotScalarUpdate::new(2, text(Some("omega")), text(Some(""))),
        ];
        if reverse {
            updates.reverse();
        }
        assert!(matches!(
            run(&before, &updates, &mut budget(), &|| false),
            Err(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(FieldConstraintViolation::EmptyText)
            )))
        ));
    }
    assert_eq!((case.before[patch], before[patch]), (1, 0));
    assert_eq!(&before[..patch], &case.before[..patch]);
    assert_eq!(&before[patch + 1..], &case.before[patch + 1..]);
}
