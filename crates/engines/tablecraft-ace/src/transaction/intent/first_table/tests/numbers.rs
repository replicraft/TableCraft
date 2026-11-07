//! Wider default target declarations use V4; V1/V2/V3 remain closed.
use super::{assert_intent_resources, budget, literal};
use crate::{CreateColumn, FirstTableDefinition, NativeFirstTableIntent, NativeImage};
use std::io::Cursor;
use tablecraft_core::values::{Decimal, Double, Single, Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn wire(tag: u8, value_tag: u8, payload: &[u8], decimal: bool) -> Vec<u8> {
    let mut raw = literal();
    raw[..8].copy_from_slice(b"TCNT0004");
    raw[12..14].copy_from_slice(&4_u16.to_le_bytes());
    raw[60..62].copy_from_slice(&2_u16.to_le_bytes());
    raw.extend([
        5,
        0,
        tag,
        3,
        0,
        0,
        if decimal { 18 } else { 0 },
        if decimal { 4 } else { 0 },
    ]);
    raw.extend(b"Value");
    raw.extend([value_tag, 0, 0, 0]);
    raw.extend((payload.len() as u32).to_le_bytes());
    raw.extend(payload);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

#[test]
fn wider_defaults_match_independent_wire_restore_exact_values_and_account_resources() {
    let decimal = Decimal::new(-123450, 4).unwrap();
    let mut decimal_wire = (-123450_i128).to_le_bytes().to_vec();
    decimal_wire.push(4);
    let cases = [
        (ScalarType::Byte, Value::Byte(255), 2, 8, vec![255]),
        (
            ScalarType::Int16,
            Value::Int16(i16::MIN),
            3,
            9,
            vec![0, 128],
        ),
        (
            ScalarType::Single,
            Value::Single(Single::from_bits(1)),
            7,
            10,
            vec![1, 0, 0, 0],
        ),
        (
            ScalarType::Double,
            Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
            8,
            11,
            vec![0, 0, 0, 0, 0, 0, 0, 128],
        ),
        (
            ScalarType::Decimal,
            Value::Decimal(decimal),
            13,
            13,
            decimal_wire,
        ),
        (ScalarType::Guid, Value::Null, 10, 0, vec![]),
    ];
    for (kind, value, field_tag, value_tag, payload) in cases {
        let primary = CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap();
        let column = if kind == ScalarType::Decimal {
            CreateColumn::decimal("Value", DecimalConstraints::new(18, 4).unwrap())
        } else {
            CreateColumn::new("Value", kind)
        }
        .with_required(false)
        .with_default_literal(&value)
        .unwrap();
        let definition =
            FirstTableDefinition::capture("Items", &[primary, column], &mut budget(), &|| false)
                .unwrap();
        let source = || {
            NativeImage::capture(
                &mut Cursor::new(canonical_blank_creation::source()),
                CatalogScope::new(491),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        };
        let plan = source()
            .plan_first_table(definition, &mut budget(), &|| false)
            .unwrap();
        let expected = wire(field_tag, value_tag, &payload, kind == ScalarType::Decimal);
        assert_eq!(
            plan.encode_intent(&mut budget(), &|| false).unwrap(),
            expected
        );
        let decoded = NativeFirstTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
        let retained = decoded
            .definition
            .borrowed_columns(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(retained[1].default_literal(), Some(&value));
        assert_eq!(retained[1].required(), Some(false));
        drop(retained);
        assert_eq!(
            source()
                .plan_first_table(decoded.definition, &mut budget(), &|| false)
                .unwrap()
                .after_bytes(),
            plan.after_bytes()
        );
        assert_intent_resources(plan, expected.clone());
        for version in [2, 3] {
            let mut legacy = expected.clone();
            legacy[7] = b'0' + version;
            legacy[12] = version;
            if version == 2 {
                legacy[91] = 1;
            }
            assert!(NativeFirstTableIntent::decode(&legacy, &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
fn legacy_wide_target_null_defaults_stay_invalid_and_v4_noncanonical_forms_refuse() {
    for (field_tag, decimal) in [
        (2, false),
        (3, false),
        (7, false),
        (8, false),
        (10, false),
        (13, true),
    ] {
        let raw = wire(field_tag, 0, &[], decimal);
        NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        for version in [2, 3] {
            let mut legacy = raw.clone();
            legacy[7] = b'0' + version;
            legacy[12] = version;
            if version == 2 {
                legacy[91] = 1;
            }
            assert!(NativeFirstTableIntent::decode(&legacy, &mut budget(), &|| false).is_err());
        }
    }
    let raw = wire(2, 8, &[255], false);
    for (offset, byte) in [(12, 3), (90, 4), (91, 2), (101, 255), (102, 1)] {
        let mut invalid = raw.clone();
        invalid[offset] = byte;
        assert!(
            NativeFirstTableIntent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
}
