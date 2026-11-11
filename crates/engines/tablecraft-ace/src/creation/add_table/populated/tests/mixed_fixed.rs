//! Independent complete mixed fixed images exercise the shared scalar encoder.
use super::{assert_image, budget, image};
use crate::{CreateColumn, FirstTableDefinition, read_table};
use std::io::Cursor;
use tablecraft_core::values::{Double, Guid, LegacyDateTime, Single, Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, populated::mixed_fixed as raw,
};

fn values(row: &raw::Row) -> Vec<Value> {
    vec![
        row.guid
            .map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
        row.date.map_or(Value::Null, |bits| {
            Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
        }),
        row.single
            .map_or(Value::Null, |bits| Value::Single(Single::from_bits(bits))),
        row.double
            .map_or(Value::Null, |bits| Value::Double(Double::from_bits(bits))),
        row.byte.map_or(Value::Null, Value::Byte),
        Value::Boolean(row.enabled),
    ]
}

#[test]
fn mixed_fixed_scalars_preserve_literal_storage_across_multiple_data_pages() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let literals = raw::rows();
    let expected = raw::after(&before, "MeasurementsCopy", &literals);
    let columns = [
        CreateColumn::new("Token", ScalarType::Guid),
        CreateColumn::new("OccurredAt", ScalarType::DateTime),
        CreateColumn::new("Ratio", ScalarType::Single),
        CreateColumn::new("Reading", ScalarType::Double),
        CreateColumn::new("Code", ScalarType::Byte),
        CreateColumn::new("Enabled", ScalarType::Boolean),
    ];
    let declaration = FirstTableDefinition::capture_manual_native(
        "MeasurementsCopy",
        &columns,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let supplied = literals.iter().map(values).collect();
    let plan = image(&before)
        .plan_populated_add_table(declaration, supplied, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.row_count(), 360);
    assert_eq!(plan.column_count(), 6);
    assert_image(plan.after_bytes(), &expected);
    // Only original global/catalog/map containers1..4 can change; every old
    // user DATA, index, property and sibling definition page remains byte-exact.
    assert_eq!(&plan.after_bytes()[..4096], &before[..4096]);
    assert_eq!(
        &plan.after_bytes()[5 * 4096..before.len()],
        &before[5 * 4096..]
    );
    assert!(plan.after_bytes().len() >= before.len() + 5 * 4096);
    let read = read_table(
        &mut Cursor::new(plan.after_bytes()),
        super::SCOPE,
        "MeasurementsCopy",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows().len(), literals.len());
    for (actual, expected) in read.rows().iter().zip(&literals) {
        assert_eq!(actual, &values(expected));
    }
}
