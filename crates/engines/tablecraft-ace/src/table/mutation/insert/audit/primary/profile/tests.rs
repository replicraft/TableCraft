use super::*;
use tablecraft_core::values::{Currency, Double, LegacyDateTime, Single};

fn profile(kind: ScalarType, width: usize, descending: bool) -> Profile {
    Profile {
        kind,
        width,
        descending,
        required: false,
        unique: false,
        ignore_nulls: false,
    }
}

#[test]
fn independent_numeric_wires_roundtrip_all_admitted_carriers_and_directions() {
    let cases = [
        (ScalarType::Byte, 1, Value::Byte(255), vec![127, 255]),
        (ScalarType::Int16, 2, Value::Int16(-1), vec![127, 127, 255]),
        (
            ScalarType::Int32,
            4,
            Value::Int32(i32::MIN),
            vec![127, 0, 0, 0, 0],
        ),
        (
            ScalarType::Int64,
            8,
            Value::Int64(i64::MAX),
            vec![127, 255, 255, 255, 255, 255, 255, 255, 255],
        ),
        (
            ScalarType::Currency,
            8,
            Value::Currency(Currency::from_scaled(10000)),
            vec![127, 128, 0, 0, 0, 0, 0, 39, 16],
        ),
    ];
    for (kind, width, value, bytes) in cases {
        for descending in [false, true] {
            let profile = profile(kind, width, descending);
            let key = profile.request(&value).unwrap();
            let mut expected = bytes.clone();
            if descending {
                for byte in &mut expected {
                    *byte = !*byte;
                }
            }
            expected.extend([1, 2, 3, 4]);
            let wire = profile.encode(key, 0x01020304).unwrap();
            assert_eq!(&*wire, expected.as_slice());
            assert_eq!(profile.value(&Wire::read(&expected).unwrap()).unwrap(), key);
        }
    }
}

#[test]
fn nullable_keys_have_real_five_byte_entries_and_required_keys_refuse_them() {
    for kind in [
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Currency,
    ] {
        for descending in [false, true] {
            let width = match kind {
                ScalarType::Byte => 1,
                ScalarType::Int16 => 2,
                ScalarType::Int32 => 4,
                _ => 8,
            };
            let profile = profile(kind, width, descending);
            let wire = profile.encode(None, 0x01020304).unwrap();
            assert_eq!(&*wire, [if descending { 255 } else { 0 }, 1, 2, 3, 4]);
            assert_eq!(profile.value(&wire).unwrap(), None);
            let required = Profile {
                required: true,
                ..profile
            };
            assert!(matches!(
                required.request(&Value::Null),
                Err(EditError::Preservation)
            ));
            assert!(matches!(
                required.value(&wire),
                Err(EditError::Preservation)
            ));
        }
    }
}

#[test]
fn source_wire_lengths_and_exact_request_carriers_are_not_coerced() {
    let selected = profile(ScalarType::Int16, 2, false);
    for wrong in [Value::Byte(1), Value::Int32(1), Value::Int64(1)] {
        assert!(matches!(
            selected.request(&wrong),
            Err(EditError::Preservation)
        ));
    }
    for bytes in [
        &[127, 0, 0, 0, 0, 0][..],
        &[128, 0, 0, 0, 0, 0, 0][..],
        &[0, 0, 0, 0, 0, 0][..],
    ] {
        assert!(matches!(
            selected.value(&Wire::read(bytes).unwrap()),
            Err(EditError::Preservation)
        ));
    }
    let widest = profile(ScalarType::Int64, 8, false);
    let leaf = widest.encode(Some(i64::MIN), 0x01020304).unwrap();
    assert_eq!(leaf.len(), 13);
    let bound = leaf.child(0x05060708).unwrap();
    assert_eq!(bound.len(), 17);
    assert_eq!(&bound[13..], &[5, 6, 7, 8]);
    assert!(matches!(bound.child(1), Err(EditError::Preservation)));
    for (kind, width, atom) in [
        (ScalarType::Byte, 1, -1),
        (ScalarType::Byte, 1, 256),
        (ScalarType::Int16, 2, 32768),
        (ScalarType::Int32, 4, 2147483648),
        (ScalarType::Single, 4, 2147483648),
    ] {
        assert!(matches!(
            profile(kind, width, false).encode(Some(atom), 1),
            Err(EditError::Preservation)
        ));
    }
}

fn assert_literal_key(kind: ScalarType, width: usize, value: &Value, ascending: &[u8]) {
    for descending in [false, true] {
        let selected = profile(kind, width, descending);
        let atom = selected.request(value).unwrap();
        let mut expected = ascending.to_vec();
        if descending {
            for byte in &mut expected {
                *byte = !*byte;
            }
        }
        expected.extend([1, 2, 3, 4]);

        let wire = selected.encode(atom, 0x01020304).unwrap();
        assert_eq!(&*wire, expected.as_slice());
        assert_eq!(
            selected.value(&Wire::read(&expected).unwrap()).unwrap(),
            atom
        );
    }
}

#[test]
fn boolean_keys_use_only_two_physical_atoms_and_never_null_or_numeric_coercion() {
    assert_literal_key(ScalarType::Boolean, 1, &Value::Boolean(true), &[127, 0]);
    assert_literal_key(ScalarType::Boolean, 1, &Value::Boolean(false), &[127, 255]);

    for descending in [false, true] {
        let selected = Profile {
            ignore_nulls: true,
            ..profile(ScalarType::Boolean, 1, descending)
        };
        assert_eq!(selected.request(&Value::Boolean(false)).unwrap(), Some(0));
        assert_eq!(selected.request(&Value::Boolean(true)).unwrap(), Some(-1));
        for wrong in [Value::Null, Value::Byte(0), Value::Int16(-1)] {
            assert!(matches!(
                selected.request(&wrong),
                Err(EditError::Preservation)
            ));
        }
        assert!(matches!(
            selected.encode(None, 1),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            selected.encode(Some(1), 1),
            Err(EditError::Preservation)
        ));

        let invalid = if descending {
            [128, 254, 0, 0, 0, 1]
        } else {
            [127, 1, 0, 0, 0, 1]
        };
        assert!(matches!(
            selected.value(&Wire::read(&invalid).unwrap()),
            Err(EditError::Preservation)
        ));
        let null = [if descending { 255 } else { 0 }, 0, 0, 0, 1];
        assert!(matches!(
            selected.value(&Wire::read(&null).unwrap()),
            Err(EditError::Preservation)
        ));
    }
}

#[test]
fn single_wire_goldens_keep_signed_zero_infinities_and_nan_payload_families() {
    for (bits, ordered) in [
        (0x80000000_u32, 0x7fffffff_u32),
        (0x00000000, 0x80000000),
        (0xff800000, 0x007fffff),
        (0x7f800000, 0xff800000),
        (0x7fc00001, 0xffc00001),
        (0x7fc00002, 0xffc00002),
        (0xffc00001, 0x003ffffe),
    ] {
        let mut expected = vec![127];
        expected.extend(ordered.to_be_bytes());
        assert_literal_key(
            ScalarType::Single,
            4,
            &Value::Single(Single::from_bits(bits)),
            &expected,
        );
    }
    let selected = profile(ScalarType::Single, 4, false);
    let key = |bits| {
        selected
            .request(&Value::Single(Single::from_bits(bits)))
            .unwrap()
    };
    assert_ne!(key(0x80000000), key(0));
    assert_ne!(key(0x7fc00001), key(0x7fc00002));
    assert!(matches!(
        selected.request(&Value::Int32(0)),
        Err(EditError::Preservation)
    ));
    assert_eq!(selected.request(&Value::Null).unwrap(), None);
}

#[test]
fn double_wire_goldens_preserve_every_bit_without_float_comparison() {
    for (bits, ordered) in [
        (0x8000000000000000_u64, 0x7fffffffffffffff_u64),
        (0x0000000000000000, 0x8000000000000000),
        (0xfff0000000000000, 0x000fffffffffffff),
        (0x7ff0000000000000, 0xfff0000000000000),
        (0x7ff8000000000001, 0xfff8000000000001),
        (0x7ff8000000000002, 0xfff8000000000002),
        (0xfff8000000000001, 0x0007fffffffffffe),
    ] {
        let mut expected = vec![127];
        expected.extend(ordered.to_be_bytes());
        assert_literal_key(
            ScalarType::Double,
            8,
            &Value::Double(Double::from_bits(bits)),
            &expected,
        );
    }
    let selected = profile(ScalarType::Double, 8, false);
    let key = |bits| {
        selected
            .request(&Value::Double(Double::from_bits(bits)))
            .unwrap()
    };
    assert_ne!(key(0x8000000000000000), key(0));
    assert_ne!(key(0x7ff8000000000001), key(0x7ff8000000000002));
    assert!(matches!(
        selected.request(&Value::Single(Single::from_bits(0))),
        Err(EditError::Preservation)
    ));
    assert_eq!(selected.request(&Value::Null).unwrap(), None);
}

#[test]
fn legacy_date_keys_keep_raw_aliases_and_low_bits_but_reject_invalid_serials() {
    for (bits, ordered) in [
        (0x8000000000000000_u64, 0x7fffffffffffffff_u64),
        (0x0000000000000000, 0x8000000000000000),
        (0xbfd0000000000000, 0x402fffffffffffff),
        (0x3fd0000000000000, 0xbfd0000000000000),
        (0x3ff0000000000001, 0xbff0000000000001),
    ] {
        let value = LegacyDateTime::from_bits(bits).unwrap();
        let mut expected = vec![127];
        expected.extend(ordered.to_be_bytes());
        assert_literal_key(ScalarType::DateTime, 8, &Value::DateTime(value), &expected);
    }
    let selected = profile(ScalarType::DateTime, 8, false);
    for invalid in [
        0xfff0000000000000_u64,
        0xfff8000000000001,
        0xffefffffffffffff,
    ] {
        let mut wire = vec![127];
        wire.extend(invalid.to_be_bytes());
        wire.extend([1, 2, 3, 4]);
        assert!(matches!(
            selected.value(&Wire::read(&wire).unwrap()),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            selected.encode(Some((invalid ^ 0x8000000000000000) as i64), 0x01020304),
            Err(EditError::Preservation)
        ));
    }
    assert!(matches!(
        selected.request(&Value::Double(Double::from_bits(0))),
        Err(EditError::Preservation)
    ));
    assert_eq!(selected.request(&Value::Null).unwrap(), None);
}
