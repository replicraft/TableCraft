use super::*;
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single};

#[test]
fn cached_scalar_carriers_keep_stored_bits_scales_and_nullable_values() {
    let cases = [
        (1, vec![0xff], Value::Boolean(true)),
        (2, vec![255], Value::Byte(255)),
        (3, (-123_i16).to_le_bytes().to_vec(), Value::Int16(-123)),
        (4, 999_i32.to_le_bytes().to_vec(), Value::Int32(999)),
        (19, i64::MAX.to_le_bytes().to_vec(), Value::Int64(i64::MAX)),
        (
            6,
            0x80000000_u32.to_le_bytes().to_vec(),
            Value::Single(Single::from_bits(0x80000000)),
        ),
        (
            7,
            (-0.0_f64).to_bits().to_le_bytes().to_vec(),
            Value::Double(Double::from_bits((-0.0_f64).to_bits())),
        ),
        (
            5,
            (-12345_i64).to_le_bytes().to_vec(),
            Value::Currency(Currency::from_scaled(-12345)),
        ),
        (
            8,
            2.5_f64.to_bits().to_le_bytes().to_vec(),
            Value::DateTime(LegacyDateTime::from_bits(2.5_f64.to_bits()).unwrap()),
        ),
        (15, vec![0; 16], Value::Guid(Guid::from_bytes([0; 16]))),
        (
            16,
            vec![14, 0, 4, 128, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0],
            Value::Decimal(Decimal::new(-12345, 4).unwrap()),
        ),
        (10, b"s\0t\0a\0l\0e\0".to_vec(), Value::Text("stale".into())),
    ];
    for profile in [5, 6] {
        for (kind, payload, expected) in &cases {
            let capacity = if *kind == 10 { 43 } else { 39 };
            let bytes = scalar_fixture::database(
                profile,
                *kind,
                capacity,
                false,
                &[Some(payload.clone()), None],
            );
            let source = source(Cursor::new(bytes), 1, &|| false, true);
            assert_eq!(
                collect(&source, &[1]).unwrap(),
                [vec![expected.clone()], vec![Value::Null]]
            );
            assert_eq!(buffered(&source, &[1]), Err(QueryError::StructureLimit));
        }
    }
}

#[test]
fn cached_memo_projection_uses_existing_bounded_long_value_decoder() {
    let text = "stored 🦀, distinct from the expression result";
    let units = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    for profile in [5, 6] {
        let bytes = memo_fixture::database(
            profile,
            false,
            &[
                Some(memo_fixture::inline(&memo_fixture::wrapped(&units))),
                None,
            ],
            &[],
        );
        let source = source(Cursor::new(bytes), 1, &|| false, true);
        assert_eq!(
            collect(&source, &[1]).unwrap(),
            [vec![Value::Text(text.into())], vec![Value::Null]]
        );
        assert_eq!(buffered(&source, &[1]), Err(QueryError::StructureLimit));
    }
}
