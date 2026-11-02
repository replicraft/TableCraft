//! Complete fixed-family images from the separately frozen SOURCE owner.
use super::{
    CreateColumn, CreateDatabaseError, create_ace16_database, create_ace16_int32_database,
};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, Value},
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_fixed_creation, canonical_fixed_float_creation,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

fn scalar(native: u8) -> ScalarType {
    match native {
        1 => ScalarType::Boolean,
        2 => ScalarType::Byte,
        3 => ScalarType::Int16,
        4 => ScalarType::Int32,
        19 => ScalarType::Int64,
        5 => ScalarType::Currency,
        6 => ScalarType::Single,
        7 => ScalarType::Double,
        8 => ScalarType::DateTime,
        15 => ScalarType::Guid,
        _ => panic!("unexpected independent SOURCE family"),
    }
}

#[test]
fn generic_fixed_creation_matches_five_complete_independent_originals() {
    for case in canonical_fixed_creation::CASES {
        let mut source = canonical_fixed_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = source
            .columns
            .iter()
            .map(|column| CreateColumn::new(column.name, scalar(column.native_type)))
            .collect::<Vec<_>>();
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let created =
            create_ace16_database(source.table, &columns, &rows, &mut budget(), &|| false).unwrap();
        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.column_count(), 10);
        assert_eq!(created.row_count(), source.rows.len());
    }
}

#[test]
fn generic_and_int32_convenience_retain_all_six_original_images() {
    for case in canonical_creation::CASES {
        let mut source = canonical_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let names = source
            .columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let columns = names
            .iter()
            .map(|name| CreateColumn::new(name, ScalarType::Int32))
            .collect::<Vec<_>>();
        let values = source
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| Value::Int32(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let rows = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let original_rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let generic =
            create_ace16_database(source.table, &columns, &rows, &mut budget(), &|| false).unwrap();
        let convenience = create_ace16_int32_database(
            source.table,
            &names,
            &original_rows,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(generic.as_bytes(), source.bytes);
        assert_eq!(convenience.as_bytes(), source.bytes);
    }
}

#[test]
fn unsupported_declarations_and_wrong_carriers_refuse_before_reservation() {
    let null_row = [Value::Null];
    for kind in [
        ScalarType::Decimal,
        ScalarType::Text,
        ScalarType::Binary,
        ScalarType::DateTimeExtended,
    ] {
        for rows in [Vec::new(), vec![null_row.as_slice()]] {
            let mut no_bytes = ResourceBudget::new(0, 100_000, 16_000_000);
            assert!(matches!(
                create_ace16_database(
                    "Items",
                    &[CreateColumn::new("Field", kind)],
                    &rows,
                    &mut no_bytes,
                    &|| false
                ),
                Err(CreateDatabaseError::UnsupportedType)
            ));
            assert_eq!(no_bytes.remaining(Resource::Bytes), 0);
        }
    }
    for (kind, value) in [
        (ScalarType::Boolean, Value::Null),
        (ScalarType::Int32, Value::Int64(10)),
        (ScalarType::Currency, Value::Int64(10)),
        (ScalarType::DateTime, Value::Double(Double::from_bits(0))),
    ] {
        let mut no_bytes = ResourceBudget::new(0, 100_000, 16_000_000);
        assert!(matches!(
            create_ace16_database(
                "Items",
                &[CreateColumn::new("Field", kind)],
                &[&[value]],
                &mut no_bytes,
                &|| false
            ),
            Err(CreateDatabaseError::Carrier)
        ));
        assert_eq!(no_bytes.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn generic_rows_retain_rectangular_and_actual_physical_fit_guards() {
    let mut source = canonical_fixed_creation::source(canonical_fixed_creation::Case::Boundary);
    source.bytes = native17::source(source.bytes);
    let columns = source
        .columns
        .iter()
        .map(|column| CreateColumn::new(column.name, scalar(column.native_type)))
        .collect::<Vec<_>>();
    let mut rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    rows.push(source.rows[0].as_slice());
    assert!(matches!(
        create_ace16_database(source.table, &columns, &rows, &mut budget(), &|| false),
        Err(CreateDatabaseError::PageCapacity)
    ));
    assert!(matches!(
        create_ace16_database(source.table, &columns, &[&[]], &mut budget(), &|| false),
        Err(CreateDatabaseError::RowWidth)
    ));
}

#[test]
fn generic_fixed_creation_preserves_independent_pools_and_final_cancellation() {
    let mut source = canonical_fixed_creation::source(canonical_fixed_creation::Case::Nullable);
    source.bytes = native17::source(source.bytes);
    let columns = source
        .columns
        .iter()
        .map(|column| CreateColumn::new(column.name, scalar(column.native_type)))
        .collect::<Vec<_>>();
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut measured = budget();
    let polls = Cell::new(0_usize);
    create_ace16_database(source.table, &columns, &rows, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let initial = budget();
    let used = [
        initial.remaining(Resource::Bytes) - measured.remaining(Resource::Bytes),
        initial.remaining(Resource::Rows) - measured.remaining(Resource::Rows),
        initial.remaining(Resource::Steps) - measured.remaining(Resource::Steps),
    ];
    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        assert!(used[position] > 0);
        let mut pools = [16 * 1024 * 1024, 100_000, 16_000_000];
        pools[position] = used[position] - 1;
        let mut limited = ResourceBudget::new(pools[0], pools[1], pools[2]);
        match create_ace16_database(source.table, &columns, &rows, &mut limited, &|| false)
            .err()
            .expect("one-low pool must refuse")
        {
            CreateDatabaseError::Inspect(InspectError::Limit(error)) => {
                assert_eq!(error.resource, resource)
            }
            other => panic!("unexpected refusal: {other:?}"),
        }
    }
    let final_poll = polls.get();
    let observed = Cell::new(0_usize);
    assert!(matches!(
        create_ace16_database(source.table, &columns, &rows, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() == final_poll
        }),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
    let mut no_allocation = budget();
    let before = no_allocation.remaining(Resource::Bytes);
    assert!(matches!(
        create_ace16_database(source.table, &columns, &rows, &mut no_allocation, &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(no_allocation.remaining(Resource::Bytes), before);
}

#[test]
fn generic_fixed_creation_retains_all_five_ieee_source_bit_patterns() {
    for case in canonical_fixed_float_creation::CASES {
        let mut source = canonical_fixed_float_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = source
            .columns
            .iter()
            .map(|column| CreateColumn::new(column.name, scalar(column.native_type)))
            .collect::<Vec<_>>();
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let created =
            create_ace16_database(source.table, &columns, &rows, &mut budget(), &|| false).unwrap();
        // Entire files bind NaN payload/signalling bits and signed infinities,
        // not merely numeric equality or a formatter's canonical NaN spelling.
        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.column_count(), 10);
        assert_eq!(created.row_count(), 2);
    }
}
