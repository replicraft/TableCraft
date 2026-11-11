//! Complete populated candidate acceptance, distinct from empty-table protocols.
use super::*;
use std::io::Cursor;
use tablecraft_core::values::Decimal;
use tablecraft_model::CatalogScope;
use tablecraft_schema::DecimalConstraints;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, populated as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(19005);

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing image byte"
    );
}

#[test]
fn complete_populated_rows_cross_the_physical_slot_boundary_without_a_global_row_quota() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = FirstTableDefinition::capture_manual_native(
        "Archive",
        &[crate::CreateColumn::new("CopiedScore", ScalarType::Int32)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let literals = (0..900)
        .map(|row| (row % 17 != 0).then_some(row))
        .collect::<Vec<_>>();
    let expected = raw::numbers(&before, "Archive", &literals);
    let values = (0..900)
        .map(|row| {
            vec![if row % 17 == 0 {
                Value::Null
            } else {
                Value::Int32(row)
            }]
        })
        .collect::<Vec<_>>();
    let plan = image(&before)
        .plan_populated_add_table(definition, values, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before.as_slice());
    assert_eq!(plan.row_count(), 900);
    assert_eq!(plan.column_count(), 1);
    assert_image(plan.after_bytes(), &expected);
    let actual = crate::read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.rows().len(), 900);
    for (position, row) in actual.rows().iter().enumerate() {
        assert_eq!(
            row,
            &[if position % 17 == 0 {
                Value::Null
            } else {
                Value::Int32(position as i32)
            }]
        );
    }
}

#[test]
fn variable_text_decimal_null_empty_and_exact_scale_have_independent_complete_images() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let columns = [
        crate::CreateColumn::short_text("Label", 7).unwrap(),
        crate::CreateColumn::decimal("Amount", DecimalConstraints::new(28, 4).unwrap()),
    ];
    let literals = (0..600)
        .map(|row| match row % 4 {
            0 => (Some("日本🦀"), Some(90_071_992_547_409_931_234)),
            1 => (Some(""), Some(-90_071_992_547_409_934_321)),
            2 => (None, Some(0)),
            _ => (Some("tail"), None),
        })
        .collect::<Vec<_>>();
    let expected = raw::text_decimals(&before, "Archive", &literals);
    let values = literals
        .iter()
        .map(|(text, decimal)| {
            vec![
                text.map_or(Value::Null, |text| Value::Text(text.into())),
                decimal.map_or(Value::Null, |coefficient| {
                    Value::Decimal(Decimal::new(coefficient, 4).unwrap())
                }),
            ]
        })
        .collect();
    let declaration =
        FirstTableDefinition::capture_manual_native("Archive", &columns, &mut budget(), &|| false)
            .unwrap();
    let plan = image(&before)
        .plan_populated_add_table(declaration, values, &mut budget(), &|| false)
        .unwrap();
    assert_image(plan.after_bytes(), &expected);
    assert_eq!(plan.row_count(), 600);
    assert!(plan.after_bytes().len() > before.len() + 2 * 4096);
}

#[test]
fn boolean_native_zero_and_empty_results_do_not_invent_nulls_or_generation() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    for count in [0, 1, 600] {
        let literals = (0..count).map(|row| row % 3 == 0).collect::<Vec<_>>();
        let expected = raw::booleans(&before, "Archive", &literals);
        let declaration = FirstTableDefinition::capture_manual_native(
            "Archive",
            &[crate::CreateColumn::new("Enabled", ScalarType::Boolean)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let values = literals
            .into_iter()
            .map(|value| vec![Value::Boolean(value)])
            .collect();
        let plan = image(&before)
            .plan_populated_add_table(declaration, values, &mut budget(), &|| false)
            .unwrap();
        assert_image(plan.after_bytes(), &expected);
        assert_eq!(plan.row_count(), count);
    }
}

#[test]
fn owned_map_crosses_real_inline_windows_with_fresh_bitmap_and_global_allocation() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let literals = (0..15_000).map(Some).collect::<Vec<_>>();
    let expected = raw::numbers(&before, "Archive", &literals);
    let values = literals
        .into_iter()
        .map(|value| vec![Value::Int32(value.unwrap())])
        .collect();
    let declaration = FirstTableDefinition::capture_manual_native(
        "Archive",
        &[crate::CreateColumn::new("CopiedScore", ScalarType::Int32)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image(&before)
        .plan_populated_add_table(declaration, values, &mut budget(), &|| false)
        .unwrap();
    assert_image(plan.after_bytes(), &expected);
    assert!(plan.after_bytes().len() > 64 * 4096);
    assert_eq!(
        &plan.after_bytes()[plan.after_bytes().len() - 4096..][..4],
        &[5, 1, 0, 0]
    );
}

#[test]
fn late_wrong_carrier_existing_destination_and_inherited_properties_return_no_candidate() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let declaration = || {
        FirstTableDefinition::capture_manual_native(
            "Archive",
            &[crate::CreateColumn::new("CopiedScore", ScalarType::Int32)],
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    let mut values = (0..70)
        .map(|value| vec![Value::Int32(value)])
        .collect::<Vec<_>>();
    values.push(vec![Value::Text("invalid carrier".into())]);
    assert!(matches!(
        image(&before).plan_populated_add_table(declaration(), values, &mut budget(), &|| false),
        Err(PopulatedAddTableError::Create(CreateDatabaseError::Carrier))
    ));
    let existing = FirstTableDefinition::capture_manual_native(
        "Table1",
        &[crate::CreateColumn::new("CopiedScore", ScalarType::Int32)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        image(&before).plan_populated_add_table(existing, Vec::new(), &mut budget(), &|| false),
        Err(PopulatedAddTableError::Append(AddTableError::DuplicateName))
    ));
    let default = Value::Int32(9);
    let inherited = crate::CreateColumn::new("CopiedScore", ScalarType::Int32)
        .with_required(false)
        .with_default_literal(&default)
        .unwrap();
    let inherited = FirstTableDefinition::capture_manual_native(
        "Archive",
        &[inherited],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        image(&before).plan_populated_add_table(inherited, Vec::new(), &mut budget(), &|| false),
        Err(PopulatedAddTableError::Declaration)
    ));
}

#[test]
fn old_bounded_construction_and_empty_request_admission_remain_separate() {
    let columns = [crate::CreateColumn::new("CopiedScore", ScalarType::Int32)];
    let rows = (0..65)
        .map(|value| vec![Value::Int32(value)])
        .collect::<Vec<_>>();
    let borrowed = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert!(matches!(
        crate::create_ace16_database("Archive", &columns, &borrowed, &mut budget(), &|| false),
        Err(CreateDatabaseError::RowLimit)
    ));
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let declaration = || {
        FirstTableDefinition::capture_manual_native("Archive", &columns, &mut budget(), &|| false)
            .unwrap()
    };
    assert!(matches!(
        image(&before).plan_populated_add_table(declaration(), Vec::new(), &mut budget(), &|| true),
        Err(PopulatedAddTableError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        matches!(image(&before).plan_populated_add_table(declaration(), Vec::new(), &mut ResourceBudget::new(u64::MAX, u64::MAX, 0), &|| false), Err(PopulatedAddTableError::Inspect(InspectError::Limit(limit))) if limit.resource == tablecraft_core::limits::Resource::Steps)
    );
    let old_source = tablecraft_testkit::fixtures::access::canonical_initial_table_creation::source(
        tablecraft_testkit::fixtures::access::canonical_initial_table_creation::Case::LongInteger,
    ).bytes;
    assert!(matches!(
        image(&old_source).plan_populated_add_table(
            declaration(),
            Vec::new(),
            &mut budget(),
            &|| false
        ),
        Err(PopulatedAddTableError::Source)
    ));
}
