use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "ace16/int64_indexes.rs"]
mod int64_indexes;
#[path = "ace16/int64_ranges.rs"]
mod int64_ranges;
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_table_schema, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn native_large_numbers_preserve_signed_extremes_above_double_precision_and_null() {
    let numbers = [
        Some(i64::MIN),
        Some(i64::MAX),
        Some(9_007_199_254_740_993),
        Some(-9_007_199_254_740_993),
        Some(0),
        None,
    ];
    let payloads = numbers
        .iter()
        .map(|value| value.map(|number| number.to_le_bytes().to_vec()))
        .collect::<Vec<_>>();
    for code in [5, 6] {
        let bytes = scalar_fixture::fixed_scalar(code, 0x13, 8, &payloads);
        let table = read_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.columns()[1].data_type(), ScalarType::Int64);
        for (row, expected) in table.rows().iter().zip(numbers) {
            assert_eq!(row[1], expected.map_or(Value::Null, Value::Int64));
        }
    }
}

#[test]
fn native_extended_datetimes_preserve_epoch_upper_boundary_fraction_and_null() {
    let values = [
        Some(b"0000000000000000000:0000000000000000000:7\0".to_vec()),
        Some(format!("{:019}:{:012}{:07}:7\0", 3_652_058, 86_399, 9_999_999).into_bytes()),
        None,
    ];
    let mut bytes = scalar_fixture::fixed_scalar(6, 0x14, 42, &values);
    let table = read_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.columns()[1].data_type(), ScalarType::DateTimeExtended);
    let Value::DateTimeExtended(first) = table.rows()[0][1] else {
        panic!("wrong carrier");
    };
    let Value::DateTimeExtended(last) = table.rows()[1][1] else {
        panic!("wrong carrier");
    };
    assert_eq!((first.date(), first.time()), ((1, 1, 1), (0, 0, 0, 0)));
    assert_eq!(
        (last.date(), last.time()),
        ((9999, 12, 31), (23, 59, 59, 9_999_999))
    );
    assert_eq!(table.rows()[2][1], Value::Null);
    // Corrupt the second timestamp after a valid first row; no partial snapshot.
    let offset = usize::from(u16::from_le_bytes(
        bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap(),
    )) & 0x1fff;
    bytes[12 * 4096 + offset + 6] = b'x';
    assert_eq!(
        read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn modern_types_are_profile_gated_and_wrong_widths_never_become_null() {
    for (code, kind, width) in [(2, 0x13, 8), (5, 0x14, 42), (6, 0x13, 7), (6, 0x14, 41)] {
        let bytes = scalar_fixture::fixed_scalar(code, kind, width, &[]);
        let schema = inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[1].native_type(), kind);
        assert!(schema.columns()[1].scalar_type().is_none());
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}
