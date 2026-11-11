//! Whole-image oracles retain exact scalar bits, original records and siblings.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker, check_table, read_table,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, LegacyDateTime, Single, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{fields, fixed_scalars as raw},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4321);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn inputs() -> [raw::Scalar; 6] {
    [
        raw::Scalar::Byte(17),
        raw::Scalar::Int16(-1234),
        raw::Scalar::Int64(9_007_199_254_740_993),
        raw::Scalar::SingleBits(0x3fa0_0000),
        raw::Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
        raw::Scalar::DateTimeBits(0x4004_0000_0000_0000),
    ]
}

fn values(input: raw::Scalar) -> Value {
    match input {
        raw::Scalar::Byte(value) => Value::Byte(value),
        raw::Scalar::Int16(value) => Value::Int16(value),
        raw::Scalar::Int64(value) => Value::Int64(value),
        raw::Scalar::SingleBits(value) => Value::Single(Single::from_bits(value)),
        raw::Scalar::DoubleBits(value) => Value::Double(Double::from_bits(value)),
        raw::Scalar::DateTimeBits(value) => {
            Value::DateTime(LegacyDateTime::from_bits(value).unwrap())
        }
    }
}

fn source() -> (Vec<u8>, u32, usize) {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(before.len() / 4096).unwrap();
    let catalog_root = usize::from(u16::from_le_bytes(
        before[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = raw::append_table(&before, "Measurements");
    let first = raw::append_values(&empty, definition, &inputs());
    (raw::append(&first, definition), definition, catalog_root)
}

fn plan(
    before: &[u8],
    column: &str,
    row: usize,
    expected: Value,
    replacement: Value,
) -> Result<NativeEditPlan, EditError> {
    let requests = [FixedScalarUpdate::new(row, expected, replacement)];
    let groups = [FittedColumnUpdates::Fixed {
        column,
        updates: &requests,
    }];
    image(before).plan_retained_fixed_scalar_row_updates(
        "Measurements",
        &groups,
        &mut budget(),
        &|| false,
    )
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

fn rows(bytes: &[u8], table: &str) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_rows()
}

#[test]
fn retained_fixed_scalar_rows_preserve_bits_null_payloads_siblings_and_long_chains() {
    let replacements = [
        raw::Scalar::Byte(u8::MAX),
        raw::Scalar::Int16(i16::MIN),
        raw::Scalar::Int64(i64::MIN),
        raw::Scalar::SingleBits(0x0000_0001),
        raw::Scalar::DoubleBits(0x0000_0000_0000_0001),
        raw::Scalar::DateTimeBits((-657_434_f64).to_bits()),
    ];
    let refills = [
        raw::Scalar::Byte(0),
        raw::Scalar::Int16(i16::MAX),
        raw::Scalar::Int64(i64::MAX),
        raw::Scalar::SingleBits(0x8000_0000),
        raw::Scalar::DoubleBits(0x8000_0000_0000_0000),
        raw::Scalar::DateTimeBits(2_958_465.5_f64.to_bits()),
    ];
    for forwarded in [false, true] {
        let (mut before, definition, catalog_root) = source();
        if forwarded {
            before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
            before = raw_forwarding_chain::extend(&before, 4, catalog_root, 20);
        }
        let native = image(&before);
        native
            .check_native_retained_fixed_scalar_rows_table("Measurements", &mut budget(), &|| false)
            .unwrap();
        native
            .check_native_retained_cell_updates_table("Measurements", &mut budget(), &|| false)
            .unwrap();
        let sibling = rows(&before, "Table1");
        let other = rows(&before, "Other");
        let unchanged_row = rows(&before, "Measurements").remove(1);
        for (position, column) in raw::COLUMNS_TO_EDIT.into_iter().enumerate() {
            let mut original = values(inputs()[position]);
            for replacement in [
                Some(replacements[position]),
                Some(replacements[position]),
                None,
                None,
                Some(refills[position]),
            ] {
                let target = replacement.map_or(Value::Null, values);
                let expected = raw::update(&before, definition, 0, column, replacement);
                let candidate =
                    plan(&before, column.name(), 0, original.clone(), target.clone()).unwrap();
                assert_eq!(candidate.changed_rows(), usize::from(original != target));
                equal_image(candidate.before_bytes(), &before);
                equal_image(candidate.after_bytes(), &expected);
                let observed = rows(candidate.after_bytes(), "Measurements");
                assert_eq!(observed[0][column.number()], target);
                assert_eq!(observed[1], unchanged_row);
                assert_eq!(rows(candidate.after_bytes(), "Table1"), sibling);
                assert_eq!(rows(candidate.after_bytes(), "Other"), other);
                check_table(
                    &mut Cursor::new(candidate.after_bytes()),
                    SCOPE,
                    "Measurements",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                original = target;
                before = expected;
            }
        }
    }
}

#[test]
fn retained_fixed_scalar_actions_keep_original_families_and_carriers_closed() {
    let (before, definition, _) = source();
    let requests = [FixedScalarUpdate::new(0, Value::Byte(17), Value::Byte(200))];
    let groups = [FittedColumnUpdates::Fixed {
        column: "ByteValue",
        updates: &requests,
    }];
    for result in [
        image(&before).plan_retained_row_updates("Measurements", &groups, &mut budget(), &|| false),
        image(&before).plan_retained_currency_row_updates(
            "Measurements",
            &groups,
            &mut budget(),
            &|| false,
        ),
        image(&before).plan_retained_boolean_row_updates(
            "Measurements",
            &groups,
            &mut budget(),
            &|| false,
        ),
    ] {
        assert!(matches!(
            result,
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    assert!(matches!(
        plan(&before, "ByteValue", 0, Value::Byte(0), Value::Byte(200)),
        Err(EditError::Conflict)
    ));
    for (column, expected, replacement) in [
        ("ByteValue", Value::Byte(17), Value::Int16(200)),
        ("Score", Value::Int32(0), Value::Int32(124)),
        ("ID", Value::Int32(1), Value::Int32(2)),
        (
            "SingleValue",
            values(inputs()[3]),
            Value::Single(Single::from_bits(0x7f80_0000)),
        ),
        (
            "DoubleValue",
            Value::Double(Double::from_bits(0x7ff8_0000_0000_0001)),
            Value::Null,
        ),
    ] {
        assert!(matches!(
            plan(&before, column, 0, expected, replacement),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    // The table-wide union retains LONG17 without granting it to family21.
    image(&before)
        .check_native_retained_rows_table("Measurements", &mut budget(), &|| false)
        .unwrap();
    let score = [FixedScalarUpdate::new(
        0,
        Value::Int32(0),
        Value::Int32(124),
    )];
    let score_group = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &score,
    }];
    let long = image(&before)
        .plan_retained_row_updates("Measurements", &score_group, &mut budget(), &|| false)
        .unwrap();
    equal_image(
        long.after_bytes(),
        &raw::update_score(&before, definition, 0, Some(124)),
    );
}

#[test]
fn retained_fixed_scalar_sources_keep_geometry_and_resource_refusals() {
    let (before, definition, _) = source();
    let field = definition as usize * 4096 + 75 + 2 * 25;
    for (relative, replacement) in [(16, 1_u16), (21, u16::MAX), (23, 2)] {
        let mut malformed = before.clone();
        malformed[field + relative..field + relative + 2]
            .copy_from_slice(&replacement.to_le_bytes());
        assert!(
            plan(
                &malformed,
                "ByteValue",
                0,
                Value::Byte(17),
                Value::Byte(200)
            )
            .is_err()
        );
    }
    let requests = [FixedScalarUpdate::new(0, Value::Byte(17), Value::Byte(200))];
    let groups = [FittedColumnUpdates::Fixed {
        column: "ByteValue",
        updates: &requests,
    }];
    assert!(matches!(
        image(&before).plan_retained_fixed_scalar_row_updates(
            "Measurements",
            &groups,
            &mut budget(),
            &|| true,
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&before).plan_retained_fixed_scalar_row_updates(
                "Measurements",
                &groups,
                &mut allowance,
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
}
