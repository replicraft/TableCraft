use super::{budget, read, scalar_fixture};
use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_calculation_declaration};
use tablecraft_core::values::{Guid, Single, Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const FIELD_OFFSET: usize = 10 * 4096 + 63 + 25;
const ORDINARY_FLAGS_OFFSET: usize = 15;
const EXTENDED_FLAGS_OFFSET: usize = 16;
const RESULT_PROPERTY_VALUE_OFFSET: usize = 8;

pub fn cases() -> Vec<(u8, Vec<u8>, Value, ScalarType)> {
    vec![
        (2, vec![255], Value::Byte(255), ScalarType::Byte),
        (
            3,
            i16::MIN.to_le_bytes().to_vec(),
            Value::Int16(i16::MIN),
            ScalarType::Int16,
        ),
        (
            19,
            i64::MAX.to_le_bytes().to_vec(),
            Value::Int64(i64::MAX),
            ScalarType::Int64,
        ),
        (
            6,
            0xffc01234_u32.to_le_bytes().to_vec(),
            Value::Single(Single::from_bits(0xffc01234)),
            ScalarType::Single,
        ),
        (
            15,
            vec![
                0x67, 0x45, 0x23, 0x01, 0xab, 0x89, 0xef, 0xcd, 1, 2, 3, 4, 5, 6, 7, 8,
            ],
            Value::Guid(Guid::from_bytes([
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 1, 2, 3, 4, 5, 6, 7, 8,
            ])),
            ScalarType::Guid,
        ),
    ]
}

#[test]
fn calculated_numeric_and_guid_carriers_keep_exact_values_and_nulls() {
    for code in [5, 6] {
        for (kind, payload, expected, scalar) in cases() {
            let bytes = scalar_fixture::database(
                code,
                kind,
                39,
                false,
                &[Some(payload), None, Some(vec![])],
            );
            let result = read(&bytes).unwrap();
            assert_eq!(result.values(), &[expected, Value::Null, Value::Null]);
            assert_eq!(result.data_type(), scalar);
            assert!(result.column().scalar_type().is_none());

            let declaration = inspect_calculation_declaration(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(declaration.data_type(), scalar);
            assert_eq!(declaration.expression().unwrap(), "[Id]*2");
        }
    }
}

#[test]
fn wrong_payload_widths_and_unknown_calculated_declarations_refuse() {
    for (kind, payload, _, _) in cases() {
        for length in [payload.len() - 1, payload.len() + 1] {
            // Empty wrappers have an independently observed NULL contract.
            if length == 0 {
                continue;
            }
            let bytes = scalar_fixture::database(6, kind, 39, false, &[Some(vec![0; length])]);
            assert_eq!(read(&bytes).err(), Some(InspectError::Malformed));
        }
        for (offset, invalid) in [
            (ORDINARY_FLAGS_OFFSET, 0x03),
            (ORDINARY_FLAGS_OFFSET, 0x82),
            (EXTENDED_FLAGS_OFFSET, 0xc1),
            (EXTENDED_FLAGS_OFFSET, 0xc4),
        ] {
            let mut bytes = scalar_fixture::database(6, kind, 39, false, &[]);
            bytes[FIELD_OFFSET + offset] = invalid;
            assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
        }
        let bytes = scalar_fixture::database(6, kind, 38, false, &[]);
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));

        let mut wrong_result = scalar_fixture::database(6, kind, 39, false, &[]);
        let result_property = wrong_result
            .windows(9)
            .position(|bytes| bytes == [9, 0, 0, 2, 1, 0, 1, 0, kind])
            .unwrap();
        wrong_result[result_property + RESULT_PROPERTY_VALUE_OFFSET] = 4;
        assert_eq!(read(&wrong_result).err(), Some(InspectError::Unsupported));

        let bytes = scalar_fixture::database(2, kind, 39, false, &[]);
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn calculated_single_preserves_all_ieee_payloads() {
    for bits in [
        0_u32, 0x80000000, 1, 0x7f7fffff, 0x7f800000, 0xff800000, 0x7fc01234,
    ] {
        let bytes = scalar_fixture::database(6, 6, 39, false, &[Some(bits.to_le_bytes().to_vec())]);
        assert_eq!(
            read(&bytes).unwrap().values(),
            &[Value::Single(Single::from_bits(bits))]
        );
    }
}
