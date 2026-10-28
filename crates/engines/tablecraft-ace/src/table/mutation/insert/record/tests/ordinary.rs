//! Literal complete rows, with fixture metadata independent of builder constants.
use super::*;
use tablecraft_core::values::{Decimal, Double, Guid, LegacyDateTime, Single};

#[test]
fn decimal_payload_charges_exact_width_and_checks_late_cancellation() {
    let mut bytes = ordinary(
        6,
        &[("ID", 4, 0, 4), ("Amount", 16, 0, 17)],
        &[(1, 0, false)],
    );
    bytes[DEFINITION + 88 + 11] = 28;
    bytes[DEFINITION + 88 + 12] = 4;
    let metadata = schema(&bytes);
    let field = &metadata.columns()[1];
    let value = Value::Decimal(Decimal::new(-1, 4).unwrap());
    let mut allowance = ResourceBudget::new(17, 1, 100);
    let encoded = payload::encode(field, &value, &mut allowance, &|| false).unwrap();
    assert_eq!(
        encoded,
        [128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]
    );

    let mut too_small = ResourceBudget::new(16, 1, 100);
    assert!(matches!(
        payload::encode(field, &value, &mut too_small, &|| false),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    payload::encode(field, &value, &mut budget(), &count).unwrap();
    for stop in 0..checkpoints.get() {
        let visits = Cell::new(0);
        let cancelled = || {
            let position = visits.get();
            visits.set(position + 1);
            position == stop
        };
        assert!(matches!(
            payload::encode(field, &value, &mut budget(), &cancelled),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    assert_eq!(
        payload::encode(field, &value, &mut budget(), &|| false).unwrap(),
        encoded,
    );
}

fn ordinary(
    code: u8,
    fields: &[(&str, u8, u16, u16)],
    variables: &[(usize, u16, bool)],
) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = base::definition(fields, 0, 11 << 8);
    support::put16(&mut definition, 43, variables.len() as u16);
    for &(position, ordinal, compressed) in variables {
        let at = 63 + position * 25;
        definition[at + 15] = 2;
        definition[at + 16] = u8::from(compressed);
        support::put16(&mut definition, at + 7, ordinal);
    }
    bytes[DEFINITION..DEFINITION + PAGE].copy_from_slice(&definition);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &[]));
    bytes
}

#[test]
fn complete_fixed_scalars_keep_offsets_bits_guid_order_and_boolean_masks() {
    for code in [5, 6] {
        let bytes = ordinary(
            code,
            &[
                ("Earlier", 4, 0, 4),
                ("Flag", 1, 0, 0),
                ("Primary", 4, 8, 4),
                ("Byte", 2, 4, 1),
                ("Short", 3, 6, 2),
                ("Wide", 19, 12, 8),
                ("Money", 5, 20, 8),
                ("Float", 6, 28, 4),
                ("Double", 7, 32, 8),
                ("Date", 8, 40, 8),
                ("Guid", 15, 48, 16),
                ("LastFlag", 1, 0, 0),
            ],
            &[],
        );
        let schema = schema(&bytes);
        let values = [
            Value::Null,
            Value::Boolean(false),
            Value::Int32(-7),
            Value::Byte(255),
            Value::Int16(i16::MIN),
            Value::Int64(i64::MAX),
            Value::Currency(Currency::from_scaled(-10000)),
            Value::Single(Single::from_bits(0x80000000)),
            Value::Double(Double::from_bits(0x3ff8000000000000)),
            Value::DateTime(LegacyDateTime::from_bits(0xc000000000000000).unwrap()),
            Value::Guid(Guid::from_bytes([
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
            ])),
            Value::Boolean(true),
        ];
        let mut expected = vec![0; 68];
        expected[..2].copy_from_slice(&[12, 0]);
        expected[6] = 255;
        expected[8..10].copy_from_slice(&[0, 0x80]);
        expected[10..14].copy_from_slice(&[0xf9, 0xff, 0xff, 0xff]);
        expected[14..22].copy_from_slice(&[255, 255, 255, 255, 255, 255, 255, 127]);
        expected[22..30].copy_from_slice(&[0xf0, 0xd8, 255, 255, 255, 255, 255, 255]);
        expected[30..34].copy_from_slice(&[0, 0, 0, 0x80]);
        expected[34..42].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0xf8, 0x3f]);
        expected[42..50].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0xc0]);
        expected[50..66].copy_from_slice(&[3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15]);
        expected[66..].copy_from_slice(&[0xfc, 0x0f]);
        let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
        let actual = profile.build(&values, &mut budget(), &|| false).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(decoded(&bytes, &schema, &actual), values);
    }
}

#[test]
fn variable_ordinals_drive_payloads_and_reverse_footer_across_mask_bytes() {
    for code in [5, 6] {
        let bytes = ordinary(
            code,
            &[
                ("TextLate", 10, 0, 510),
                ("Earlier", 4, 0, 4),
                ("BinaryFirst", 9, 0, 510),
                ("Primary", 4, 4, 4),
                ("TextFirst", 10, 0, 510),
                ("BinaryLate", 9, 0, 510),
                ("False", 1, 0, 0),
                ("True", 1, 0, 0),
                ("Byte", 2, 8, 1),
            ],
            &[(0, 2, true), (2, 0, false), (4, 1, false), (5, 3, false)],
        );
        let schema = schema(&bytes);
        let values = [
            Value::Text("abc".into()),
            Value::Null,
            Value::Binary(vec![0xde, 0xad]),
            Value::Int32(123),
            Value::Text("Ā😀".into()),
            Value::Null,
            Value::Boolean(false),
            Value::Boolean(true),
            Value::Byte(42),
        ];
        let expected = vec![
            9, 0, 0, 0, 0, 0, 123, 0, 0, 0, 42, 0xde, 0xad, 0, 1, 0x3d, 0xd8, 0, 0xde, 0xff, 0xfe,
            b'a', b'b', b'c', 24, 0, 24, 0, 19, 0, 13, 0, 11, 0, 4, 0, 0x9d, 1,
        ];
        let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
        let row = profile.build(&values, &mut budget(), &|| false).unwrap();
        assert_eq!(row, expected);
        assert_eq!(decoded(&bytes, &schema, &row), values);
    }
}

#[test]
fn canonical_text_preserves_ties_markers_null_and_empty_without_coercion() {
    for compressed in [false, true] {
        let bytes = ordinary(
            6,
            &[("ID", 4, 0, 4), ("Text", 10, 0, 510)],
            &[(1, 0, compressed)],
        );
        let schema = schema(&bytes);
        let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
        for (text, raw, narrow) in [
            ("", vec![], vec![]),
            ("AB", vec![65, 0, 66, 0], vec![65, 0, 66, 0]),
            ("ABC", vec![65, 0, 66, 0, 67, 0], vec![255, 254, 65, 66, 67]),
            ("\0A", vec![0, 0, 65, 0], vec![0, 0, 65, 0]),
            ("\u{fffe}A", vec![254, 255, 65, 0], vec![254, 255, 65, 0]),
        ] {
            let payload = if compressed { narrow } else { raw };
            let mut expected = vec![2, 0, 1, 0, 0, 0];
            expected.extend_from_slice(&payload);
            expected.extend_from_slice(&((6 + payload.len()) as u16).to_le_bytes());
            expected.extend_from_slice(&[6, 0, 1, 0, 3]);
            let values = [Value::Int32(1), Value::Text(text.into())];
            let row = profile.build(&values, &mut budget(), &|| false).unwrap();
            assert_eq!(row, expected);
            assert_eq!(decoded(&bytes, &schema, &row), values);
        }
        let absent = profile
            .build(&[Value::Int32(1), Value::Null], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(absent, vec![2, 0, 1, 0, 0, 0, 6, 0, 6, 0, 1, 0, 1]);
        assert_eq!(
            profile.build(
                &[Value::Int32(1), Value::Text("\u{feff}A".into())],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn nullable_fixed_cells_do_not_turn_boolean_null_into_false() {
    let bytes = ordinary(5, &[("ID", 4, 0, 4), ("Flag", 1, 0, 0)], &[]);
    let schema = schema(&bytes);
    let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
    for (value, bit) in [(false, 0), (true, 2)] {
        let values = [Value::Null, Value::Boolean(value)];
        let row = profile.build(&values, &mut budget(), &|| false).unwrap();
        assert_eq!(row, vec![2, 0, 0, 0, 0, 0, bit]);
        assert_eq!(decoded(&bytes, &schema, &row), values);
    }
    assert_eq!(
        profile.build(&[Value::Int32(1), Value::Null], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    );
}

#[test]
fn physical_overlap_variable_holes_and_boolean_phantom_extent_refuse() {
    let good = ordinary(
        6,
        &[("ID", 4, 0, 4), ("Other", 4, 4, 4), ("Flag", 1, 2, 0)],
        &[],
    );
    let schema = metadata_schema(&good);
    assert!(Profile::bind(&schema, &mut budget(), &|| false).is_ok());
    for (offset, value) in [(63 + 25 + 21, 2), (63 + 50 + 21, 20)] {
        let mut bytes = good.clone();
        support::put16(&mut bytes, DEFINITION + offset, value);
        let schema = metadata_schema(&bytes);
        assert!(matches!(
            Profile::bind(&schema, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
    let good = ordinary(
        6,
        &[("ID", 4, 0, 4), ("A", 9, 0, 510), ("B", 10, 0, 510)],
        &[(1, 0, false), (2, 1, false)],
    );
    let schema = metadata_schema(&good);
    assert!(Profile::bind(&schema, &mut budget(), &|| false).is_ok());

    // Duplicate physical variable ordinals are already malformed metadata.
    // This case must refuse at the parser, before any codec is constructed.
    let mut duplicate = good.clone();
    support::put16(&mut duplicate, DEFINITION + 63 + 50 + 7, 0);
    assert!(matches!(
        inspect_table_schema(
            &mut Cursor::new(&duplicate),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed)
    ));

    // An extra declared variable slot is readable metadata. The complete-row
    // codec must independently refuse that unassigned physical ordinal.
    let mut hole = good;
    support::put16(&mut hole, DEFINITION + 43, 3);
    let schema = metadata_schema(&hole);
    assert!(matches!(
        Profile::bind(&schema, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn complete_record_capacity_and_declared_binary_limit_remain_checked() {
    let fields = [
        ("A", 9, 0, 510),
        ("B", 9, 0, 510),
        ("C", 9, 0, 510),
        ("D", 9, 0, 510),
        ("E", 9, 0, 510),
        ("F", 9, 0, 510),
        ("G", 9, 0, 510),
        ("H", 9, 0, 510),
        ("I", 9, 0, 510),
    ];
    let variables: Vec<_> = (0..9)
        .map(|position| (position, position as u16, false))
        .collect();
    let bytes = ordinary(6, &fields, &variables);
    let schema = schema(&bytes);
    let profile = Profile::bind(&schema, &mut budget(), &|| false).unwrap();
    let values: Vec<_> = (0..9).map(|_| Value::Binary(vec![0xa5; 510])).collect();
    assert_eq!(
        profile.build(&values, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    let mut values: Vec<_> = (0..9).map(|_| Value::Null).collect();
    values[0] = Value::Binary(vec![0; 511]);
    assert_eq!(
        profile.build(&values, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    );
}
