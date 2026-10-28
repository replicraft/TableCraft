use super::*;
use crate::{format::reader::Reader, inspect_table_schema, read_table, test_support::base};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{LimitError, Resource},
    values::Currency,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;

fn budget() -> ResourceBudget {
    ResourceBudget::new(8 * 1024 * 1024, 100_000, 1_000_000)
}

fn source(code: u8, reversed: bool, fixed_offset: u16, renamed: bool) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let id = if renamed { "KeyNumber" } else { "ID" };
    let binary = if renamed { "RawValue" } else { "Bytes" };
    let fields = if reversed {
        [(binary, 9, 0, 255), (id, 4, fixed_offset, 4)]
    } else {
        [(id, 4, fixed_offset, 4), (binary, 9, 0, 255)]
    };
    let mut definition = base::definition(&fields, 0, 11 << 8);
    // The old generic fixture only treats Text/OLE as variable. This new
    // ShortBinary definition explicitly owns its native variable metadata.
    let position = usize::from(!reversed);
    let at = 63 + position * 25;
    definition[at + 15] = 2;
    support::put16(&mut definition, at + 7, 0);
    support::put16(&mut definition, 43, 1);
    bytes[DEFINITION..DEFINITION + PAGE].copy_from_slice(&definition);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &[]));
    bytes
}

fn metadata_schema(bytes: &[u8]) -> TableSchema {
    inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn schema(bytes: &[u8]) -> TableSchema {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(rows.rows().is_empty());
    metadata_schema(bytes)
}

fn decoded(bytes: &[u8], schema: &TableSchema, row: &[u8]) -> Vec<Value> {
    let mut cursor = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut cursor,
        header: schema.header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    crate::table::record::decode(row, schema, &mut reader).unwrap()
}

#[test]
fn literal_null_empty_and_full_binary_rows_have_distinct_masks_and_exact_footers() {
    for code in [5, 6] {
        for reversed in [false, true] {
            let bytes = source(code, reversed, 0, false);
            let schema = schema(&bytes);
            let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
            for (binary, present) in [
                (Value::Null, false),
                (Value::Binary(Vec::new()), true),
                (Value::Binary((0..255).collect()), true),
            ] {
                let payload = match &binary {
                    Value::Binary(payload) => payload.as_slice(),
                    _ => &[],
                };
                let mut expected = vec![2, 0, 0x11, 0x27, 0, 0];
                expected.extend_from_slice(payload);
                let end = 6 + payload.len();
                expected.extend((end as u16).to_le_bytes());
                let mask = if present {
                    3
                } else if reversed {
                    2
                } else {
                    1
                };
                expected.extend([6, 0, 1, 0, mask]);
                let values = if reversed {
                    [binary, Value::Int32(10001)]
                } else {
                    [Value::Int32(10001), binary]
                };
                let row = profile.build(&values, &mut budget(), &|| false).unwrap();
                assert_eq!(row, expected);
                assert_eq!(row.len(), end + 7);
                assert_eq!(decoded(&bytes, &schema, &row), values);
            }
        }
    }
}

#[test]
fn actual_field_numbers_order_names_and_fixed_offset_drive_new_row_geometry() {
    for reversed in [false, true] {
        for offset in [0, 4] {
            let bytes = source(6, reversed, offset, true);
            let schema = schema(&bytes);
            let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
            let values = if reversed {
                [Value::Null, Value::Int32(i32::MIN)]
            } else {
                [Value::Int32(i32::MIN), Value::Null]
            };
            let row = profile.build(&values, &mut budget(), &|| false).unwrap();
            let mut expected = vec![2, 0];
            expected.extend(vec![0; offset as usize]);
            expected.extend([0, 0, 0, 0x80]);
            let end = 6 + offset;
            expected.extend(end.to_le_bytes());
            expected.extend(end.to_le_bytes());
            expected.extend([1, 0, if reversed { 2 } else { 1 }]);
            assert_eq!(row, expected);
            assert_eq!(decoded(&bytes, &schema, &row), values);
        }
    }
}

#[test]
fn complete_arity_exact_carriers_and_declared_binary_width_are_required() {
    let bytes = source(5, false, 0, false);
    let schema = schema(&bytes);
    let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
    let wrong = [
        vec![],
        vec![Value::Int32(1)],
        vec![Value::Int32(1), Value::Null, Value::Null],
        vec![Value::Int64(1), Value::Null],
        vec![Value::Currency(Currency::from_scaled(1)), Value::Null],
        vec![Value::Int32(1), Value::Text(String::new())],
        vec![Value::Int32(1), Value::Int32(0)],
        vec![Value::Int32(1), Value::Binary(vec![0; 256])],
    ];
    for values in wrong {
        assert_eq!(
            profile.build(&values, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        );
    }
    let values = [Value::Int32(i32::MAX), Value::Binary(vec![0xff; 255])];
    let row = profile.build(&values, &mut budget(), &|| false).unwrap();
    assert_eq!(&row[2..6], &[0xff, 0xff, 0xff, 0x7f]);
    assert_eq!(decoded(&bytes, &schema, &row), values);
}

#[test]
fn generated_extended_and_historical_declarations_do_not_borrow_row_authority() {
    for (offset, value, width, blocker) in [
        (63 + 15, 7, 1, WriteBlocker::Column),
        (63 + 16, 0x80, 1, WriteBlocker::Column),
        (88 + 16, 0x10, 1, WriteBlocker::Column),
        (88, 11, 1, WriteBlocker::Column),
        (41, 3, 2, WriteBlocker::RowLayout),
        (43, 2, 2, WriteBlocker::RowLayout),
    ] {
        let mut bytes = source(6, false, 0, false);
        if width == 1 {
            bytes[DEFINITION + offset] = value as u8;
        } else {
            support::put16(&mut bytes, DEFINITION + offset, value);
        }
        // Unsupported declarations still have actual inspectable metadata.
        // Do not borrow ordinary scalar read admission for generated profiles.
        let schema = metadata_schema(&bytes);
        assert!(
            matches!(Profile::bind(&schema, &mut budget(), &|| false), Err(EditError::Unsupported(actual)) if actual == blocker)
        );
    }
}

#[test]
fn record_allocation_work_and_late_cancellation_fail_without_a_partial_result() {
    let bytes = source(6, false, 0, false);
    let schema = schema(&bytes);
    let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
    let values = [Value::Int32(1), Value::Binary(vec![0xa7; 255])];
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut allowance = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 1_000_000),
            Resource::Steps => ResourceBudget::new(8 * 1024 * 1024, 100_000, 0),
            _ => unreachable!(),
        };
        assert!(
            matches!(profile.build(&values, &mut allowance, &|| false), Err(EditError::Inspect(InspectError::Limit(LimitError { resource: actual, .. }))) if actual == resource)
        );
    }
    assert_eq!(
        profile.build(&values, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    );
    let calls = Cell::new(0);
    let cancelled = || {
        let prior = calls.get();
        calls.set(prior + 1);
        prior >= 2
    };
    assert_eq!(
        profile.build(&values, &mut budget(), &cancelled),
        Err(EditError::Inspect(InspectError::Cancelled))
    );
    let row = profile.build(&values, &mut budget(), &|| false).unwrap();
    assert_eq!(row.len(), 268);
    assert_eq!(decoded(&bytes, &schema, &row), values);
}

mod ordinary;
