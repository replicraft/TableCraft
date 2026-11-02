//! Whole-image variable creation checks against independent original SOURCEs.
use super::{CreateColumn, CreateDatabaseError, create_ace16_database};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;
use tablecraft_testkit::fixtures::access::canonical_variable_creation;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

fn columns() -> [CreateColumn<'static>; 5] {
    [
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::short_text("TextValue", 8).unwrap(),
        CreateColumn::short_binary("BinaryValue", 16).unwrap(),
        CreateColumn::decimal("DecimalValue", DecimalConstraints::new(28, 4).unwrap()),
        CreateColumn::new("Flag", ScalarType::Boolean),
    ]
}

#[test]
fn declared_variable_creation_matches_all_five_independent_complete_sources() {
    for case in canonical_variable_creation::CASES {
        let mut source = canonical_variable_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let created =
            create_ace16_database(source.table, &columns(), &rows, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.row_count(), source.rows.len());
        assert_eq!(created.column_count(), 5);
    }
}

#[test]
fn explicit_metadata_is_validated_for_empty_and_all_null_inputs_without_inference() {
    for limit in [0, 256, usize::MAX] {
        assert!(matches!(
            CreateColumn::short_text("T", limit),
            Err(CreateDatabaseError::Declaration)
        ));
        assert!(matches!(
            CreateColumn::short_binary("B", limit),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    for scalar in [ScalarType::Text, ScalarType::Binary, ScalarType::Decimal] {
        let declaration = [CreateColumn::new("Unbound", scalar)];
        let null = [Value::Null];
        for rows in [&[][..], &[null.as_slice()][..]] {
            assert!(matches!(
                create_ace16_database("Items", &declaration, rows, &mut budget(), &|| false),
                Err(CreateDatabaseError::UnsupportedType)
            ));
        }
    }
    let text = CreateColumn::short_text("T", 8).unwrap();
    let binary = CreateColumn::short_binary("B", 16).unwrap();
    let constraints = DecimalConstraints::new(28, 4).unwrap();
    let decimal = CreateColumn::decimal("D", constraints);
    assert_eq!(text.max_text_utf16_units(), Some(8));
    assert_eq!(binary.max_binary_bytes(), Some(16));
    assert_eq!(decimal.decimal_constraints(), Some(constraints));
}

#[test]
fn declared_value_failures_precede_any_plan_or_image_allocation() {
    let text = [CreateColumn::short_text("T", 8).unwrap()];
    let binary = [CreateColumn::short_binary("B", 16).unwrap()];
    let decimal = [CreateColumn::decimal(
        "D",
        DecimalConstraints::new(3, 2).unwrap(),
    )];
    for (declarations, value) in [
        (text.as_slice(), Value::Text("🙂🙂🙂🙂a".into())),
        (binary.as_slice(), Value::Binary(vec![1; 17])),
        (
            decimal.as_slice(),
            Value::Decimal(Decimal::new(1, 1).unwrap()),
        ),
        (
            decimal.as_slice(),
            Value::Decimal(Decimal::new(1000, 2).unwrap()),
        ),
    ] {
        let row = [value];
        assert!(matches!(
            create_ace16_database(
                "Items",
                declarations,
                &[row.as_slice()],
                &mut ResourceBudget::new(0, 100_000, 16_000_000),
                &|| false,
            ),
            Err(CreateDatabaseError::DeclaredValue)
        ));
    }
    let unrepresentable = [Value::Text("\u{feff}\u{0100}".into())];
    assert!(matches!(
        create_ace16_database(
            "Items",
            &text,
            &[unrepresentable.as_slice()],
            &mut ResourceBudget::new(0, 100_000, 16_000_000),
            &|| false,
        ),
        Err(CreateDatabaseError::TextEncoding)
    ));
    let wrong = [Value::Binary(Vec::new())];
    assert!(matches!(
        create_ace16_database("Items", &text, &[wrong.as_slice()], &mut budget(), &|| {
            false
        }),
        Err(CreateDatabaseError::Carrier)
    ));
}

#[test]
fn variable_payload_work_obeys_all_three_pools_and_late_cancellation() {
    let mut source =
        canonical_variable_creation::source(canonical_variable_creation::Case::Unicode);
    source.bytes = native17::source(source.bytes);
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for mut limited in [
        ResourceBudget::new(0, 100_000, 16_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 16_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            create_ace16_database(source.table, &columns(), &rows, &mut limited, &|| false),
            Err(CreateDatabaseError::Inspect(_))
        ));
    }
    let polls = Cell::new(0);
    create_ace16_database(source.table, &columns(), &rows, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let final_poll = polls.get();
    polls.set(0);
    assert!(matches!(
        create_ace16_database(source.table, &columns(), &rows, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
}
