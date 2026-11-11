//! Independent exact GUID/Decimal images cover complete retained placement.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    SlotScalarUpdate, WriteBlocker, check_table, read_table,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{exact_values as raw, fields},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4322);
const TOKEN: [u8; 16] = [
    0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
];
const REPLACEMENT: [u8; 16] = [
    254, 220, 186, 152, 118, 84, 50, 16, 1, 35, 69, 103, 137, 171, 205, 239,
];
const AMOUNT: i128 = 90_071_992_547_409_931_234;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

fn source() -> (Vec<u8>, u32, usize) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let empty = raw::append_table(&seed, "ExactValues");
    let first = raw::append(&empty, definition, Some(TOKEN), Some(AMOUNT));
    (
        raw::append(&first, definition, None, None),
        definition,
        catalog_root,
    )
}

fn plan(
    before: &[u8],
    column: &str,
    row: usize,
    expected: Value,
    value: Value,
) -> Result<NativeEditPlan, EditError> {
    if column == "Amount" {
        let updates = [SlotScalarUpdate::new(row, expected, value)];
        let groups = [FittedColumnUpdates::Variable {
            column,
            updates: &updates,
        }];
        image(before).plan_retained_wide_value_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        )
    } else {
        let updates = [FixedScalarUpdate::new(row, expected, value)];
        let groups = [FittedColumnUpdates::Fixed {
            column,
            updates: &updates,
        }];
        image(before).plan_retained_wide_value_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        )
    }
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

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_wide_value_rows_preserve_exact_values_nulls_padding_and_long_chains() {
    for forwarded in [false, true] {
        let (mut before, definition, catalog_root) = source();
        if forwarded {
            before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
            before = raw_forwarding_chain::extend(&before, 4, catalog_root, 20);
        }
        image(&before)
            .check_native_retained_wide_value_rows_table("ExactValues", &mut budget(), &|| false)
            .unwrap();
        image(&before)
            .check_native_retained_cell_updates_table("ExactValues", &mut budget(), &|| false)
            .unwrap();
        let sibling = rows(&before, "Table1");
        let other = rows(&before, "Other");
        let unchanged = raw::row(None, None, 2);
        let mut expected_values = raw::row(Some(TOKEN), Some(AMOUNT), 1);
        for (column, value) in [
            ("Token", Value::Guid(Guid::from_bytes(REPLACEMENT))),
            ("Token", Value::Guid(Guid::from_bytes(REPLACEMENT))),
            ("Token", Value::Null),
            ("Token", Value::Null),
            ("Token", Value::Guid(Guid::from_bytes([0; 16]))),
            ("Amount", decimal(-90_071_992_547_409_934_321, 4)),
            ("Amount", Value::Null),
            ("Amount", Value::Null),
            ("Amount", decimal(9_999_999_999_999_999_999_999_999_999, 4)),
            ("Amount", decimal(0, 4)),
        ] {
            let position = if column == "Token" { 2 } else { 3 };
            let original = expected_values[position].clone();
            let expected = match (column, &value) {
                ("Token", Value::Guid(value)) => {
                    raw::update_guid(&before, definition, 0, Some(value.to_bytes()))
                }
                ("Token", Value::Null) => raw::update_guid(&before, definition, 0, None),
                ("Amount", Value::Decimal(value)) => {
                    raw::update_decimal(&before, definition, 0, Some(value.coefficient()))
                }
                ("Amount", Value::Null) => raw::update_decimal(&before, definition, 0, None),
                _ => unreachable!(),
            };
            let candidate = plan(&before, column, 0, original.clone(), value.clone()).unwrap();
            assert_eq!(candidate.changed_rows(), usize::from(original != value));
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &expected);
            let index = (definition as usize + 2) * 4096;
            assert_eq!(
                &candidate.after_bytes()[index..index + 4096],
                &before[index..index + 4096]
            );
            expected_values[position] = value;
            assert_eq!(
                rows(candidate.after_bytes(), "ExactValues"),
                vec![expected_values.clone(), unchanged.clone()]
            );
            assert_eq!(rows(candidate.after_bytes(), "Table1"), sibling);
            assert_eq!(rows(candidate.after_bytes(), "Other"), other);
            check_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "ExactValues",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            before = expected;
        }
    }
}

#[test]
fn retained_wide_value_actions_keep_native_kinds_families_and_expected_null_closed() {
    let (before, definition, _) = source();
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Guid(Guid::from_bytes(TOKEN)),
        Value::Guid(Guid::from_bytes(REPLACEMENT)),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Token",
        updates: &updates,
    }];
    for result in [
        image(&before).plan_retained_row_updates("ExactValues", &groups, &mut budget(), &|| false),
        image(&before).plan_retained_opaque_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        ),
        image(&before).plan_retained_currency_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        ),
        image(&before).plan_retained_boolean_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        ),
        image(&before).plan_retained_fixed_scalar_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false,
        ),
    ] {
        assert!(result.is_err());
    }
    for (column, row, expected, value) in [
        (
            "Token",
            0,
            Value::Null,
            Value::Guid(Guid::from_bytes(REPLACEMENT)),
        ),
        (
            "Token",
            1,
            Value::Guid(Guid::from_bytes(TOKEN)),
            Value::Null,
        ),
        ("Amount", 0, Value::Null, decimal(1, 4)),
        ("Amount", 1, decimal(0, 4), Value::Null),
    ] {
        assert!(matches!(
            plan(&before, column, row, expected, value),
            Err(EditError::Conflict)
        ));
    }
    for (column, expected, value) in [
        (
            "Token",
            Value::Guid(Guid::from_bytes(TOKEN)),
            Value::Binary(vec![0; 16]),
        ),
        ("Amount", decimal(AMOUNT, 4), decimal(1, 3)),
        ("Amount", decimal(AMOUNT, 4), decimal(10_i128.pow(28), 4)),
        ("Amount", decimal(AMOUNT, 3), Value::Null),
        ("Score", Value::Int32(0), Value::Int32(124)),
        ("ID", Value::Int32(1), Value::Int32(2)),
    ] {
        assert!(matches!(
            plan(&before, column, 0, expected, value),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Guid(Guid::from_bytes(TOKEN)),
        Value::Null,
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: "Token",
        updates: &updates,
    }];
    assert!(matches!(
        image(&before).plan_retained_wide_value_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    let score = [FixedScalarUpdate::new(
        0,
        Value::Int32(0),
        Value::Int32(124),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &score,
    }];
    let candidate = image(&before)
        .plan_retained_row_updates("ExactValues", &groups, &mut budget(), &|| false)
        .unwrap();
    equal_image(
        candidate.after_bytes(),
        &raw::update_score(&before, definition, 0, Some(124)),
    );
}

#[test]
fn retained_wide_value_decimal_metadata_boundaries_and_resources_remain_checked() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let empty = raw::append_table(&seed, "ExactValues");
    let zero = raw::append_omitted(&empty, definition);
    let decimal_at = definition as usize * 4096 + 75 + 3 * 25;
    for (precision, scale, coefficient) in [(1, 0, 9), (28, 28, 1)] {
        let mut before = zero.clone();
        let mut expected = raw::update_decimal(&zero, definition, 0, Some(coefficient));
        for bytes in [&mut before, &mut expected] {
            bytes[decimal_at + 11] = precision;
            bytes[decimal_at + 12] = scale;
        }
        let candidate = plan(
            &before,
            "Amount",
            0,
            decimal(0, scale),
            decimal(coefficient, scale),
        )
        .unwrap();
        equal_image(candidate.after_bytes(), &expected);
        assert_eq!(
            rows(candidate.after_bytes(), "ExactValues")[0][3],
            decimal(coefficient, scale)
        );
    }
    for (precision, scale) in [(0, 0), (29, 4), (4, 5)] {
        let mut before = zero.clone();
        before[decimal_at + 11] = precision;
        before[decimal_at + 12] = scale;
        assert!(plan(&before, "Amount", 0, decimal(0, 4), decimal(1, 4)).is_err());
    }
    let guid_at = definition as usize * 4096 + 75 + 2 * 25;
    for (at, bytes) in [
        (guid_at + 15, vec![7]),
        (guid_at + 16, vec![1]),
        (guid_at + 23, vec![8, 0]),
        (decimal_at + 15, vec![3]),
        (decimal_at + 16, vec![1]),
        (decimal_at + 23, vec![16, 0]),
    ] {
        let mut before = zero.clone();
        before[at..at + bytes.len()].copy_from_slice(&bytes);
        let (column, expected, value) = if at < decimal_at {
            ("Token", Value::Null, Value::Guid(Guid::from_bytes(TOKEN)))
        } else {
            ("Amount", decimal(0, 4), decimal(1, 4))
        };
        assert!(plan(&before, column, 0, expected, value).is_err());
    }
    // Native negative zero remains byte-identical for a semantic Decimal no-op.
    let mut negative_zero = zero.clone();
    let data = (definition as usize + 1) * 4096;
    let start = usize::from(u16::from_le_bytes(
        negative_zero[data + 14..data + 16].try_into().unwrap(),
    )) & 0x1fff;
    negative_zero[data + start + 26] = 128;
    let candidate = plan(&negative_zero, "Amount", 0, decimal(0, 4), decimal(0, 4)).unwrap();
    assert_eq!(candidate.changed_rows(), 0);
    equal_image(candidate.after_bytes(), &negative_zero);
    let updates = [SlotScalarUpdate::new(0, decimal(0, 4), decimal(1, 4))];
    let groups = [FittedColumnUpdates::Variable {
        column: "Amount",
        updates: &updates,
    }];
    assert!(matches!(
        image(&zero).plan_retained_wide_value_row_updates(
            "ExactValues",
            &groups,
            &mut budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&zero).plan_retained_wide_value_row_updates(
                "ExactValues",
                &groups,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
}
