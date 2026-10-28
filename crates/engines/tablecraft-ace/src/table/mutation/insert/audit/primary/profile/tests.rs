use super::*;
use tablecraft_core::values::Currency;

fn profile(kind: ScalarType, width: usize, descending: bool) -> Profile {
    Profile {
        kind,
        width,
        descending,
        required: false,
        unique: false,
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
}
