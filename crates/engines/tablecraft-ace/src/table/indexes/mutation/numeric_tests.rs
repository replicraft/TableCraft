use super::*;
use tablecraft_schema::ScalarType;
#[test]
fn literal_boolean_ieee_and_date_goldens_preserve_physical_domains() {
    use crate::table::indexes::composite::DecimalValue;
    for (kind, stored, expected) in [
        (
            ScalarType::Single,
            vec![0, 0, 0, 0],
            vec![127, 128, 0, 0, 0],
        ),
        (
            ScalarType::Single,
            vec![0, 0, 0, 128],
            vec![127, 127, 255, 255, 255],
        ),
        (
            ScalarType::Double,
            vec![1, 0, 0, 0, 0, 0, 248, 127],
            vec![127, 255, 248, 0, 0, 0, 0, 0, 1],
        ),
        (
            ScalarType::Double,
            vec![2, 0, 0, 0, 0, 0, 248, 255],
            vec![127, 0, 7, 255, 255, 255, 255, 255, 253],
        ),
        (
            ScalarType::DateTime,
            vec![0, 0, 0, 0, 0, 0, 208, 191],
            vec![127, 64, 47, 255, 255, 255, 255, 255, 255],
        ),
    ] {
        let codec = MutationKey::new(kind).unwrap();
        let key = codec.stored(&stored).unwrap();
        let mut bytes = [0; 9];
        let length = codec.encode(Some(key), false, &mut bytes).unwrap();
        assert_eq!(&bytes[..length], expected);
        assert!(codec.stored(&stored[..stored.len() - 1]).is_err());
    }
    let boolean = MutationKey::new(ScalarType::Boolean).unwrap();
    for (bit, expected) in [(false, [127, 255]), (true, [127, 0])] {
        let atom = boolean.cell(&[], bit).unwrap();
        assert!(matches!(atom, Some(DecimalValue::Numeric(_))));
        let mut bytes = [0; 2];
        assert_eq!(boolean.encode(atom, false, &mut bytes).unwrap(), 2);
        assert_eq!(bytes, expected);
    }
    assert!(boolean.encode(None, false, &mut [0; 2]).is_err());
    assert!(
        MutationKey::new(ScalarType::DateTime)
            .unwrap()
            .stored(&f64::INFINITY.to_bits().to_le_bytes())
            .is_err()
    );
}
