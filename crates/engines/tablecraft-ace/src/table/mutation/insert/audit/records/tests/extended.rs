//! Literal fresh record framing is separate from the production timestamp codec.
use super::{FIELD_BYTES, FIELDS, TABLE, budget, qualify, reconstruct, schema, source};
use crate::{EditError, WriteBlocker};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};
use tablecraft_testkit::access as literal;

#[test]
fn extended_timestamp_records_preserve_exact_fraction_digits_and_null_mask() {
    let bytes = source(
        &[("ID", 4, 0, 4), ("Group", 4, 4, 4), ("Stamp", 20, 8, 42)],
        &[],
    );
    let metadata = schema(&bytes);
    qualify(&metadata, &mut budget(), &|| false).unwrap();
    let value = ExtendedDateTime::new(2000, 2, 29, 12, 34, 56, 1_234_567).unwrap();
    let mut expected = vec![3, 0, 7, 0, 0, 0, 0, 0, 0, 0];
    expected.extend(b"0000000000000730178:0000000452961234567:7\0");
    expected.push(5);
    assert_eq!(expected.len(), 53);
    assert_eq!(
        reconstruct(
            &bytes,
            &metadata,
            0,
            vec![Value::Int32(7), Value::Null, Value::DateTimeExtended(value)],
        )
        .unwrap(),
        expected
    );

    let mut null = vec![3, 0, 7, 0, 0, 0, 0, 0, 0, 0];
    null.extend([0; 42]);
    null.push(1);
    assert_eq!(
        reconstruct(
            &bytes,
            &metadata,
            0,
            vec![Value::Int32(7), Value::Null, Value::Null],
        )
        .unwrap(),
        null
    );
}

#[test]
fn extended_timestamp_requires_its_carrier_and_exact_ace2019_declaration() {
    let bytes = source(&[("ID", 4, 0, 4), ("Stamp", 20, 4, 42)], &[]);
    let metadata = schema(&bytes);
    qualify(&metadata, &mut budget(), &|| false).unwrap();
    for value in [
        Value::Int64(0),
        Value::Text("2000-02-29".into()),
        Value::Binary(vec![0; 42]),
        Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
    ] {
        assert!(matches!(
            reconstruct(&bytes, &metadata, 0, vec![Value::Int32(7), value]),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }

    let mut earlier = bytes.clone();
    earlier[20] = 5;
    assert!(matches!(
        qualify(&schema(&earlier), &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    for (width, flags, extended) in [(41, 3, 0), (43, 3, 0), (42, 7, 0), (42, 3, 16)] {
        let mut invalid = bytes.clone();
        let field = TABLE + FIELDS + FIELD_BYTES;
        literal::put16(&mut invalid, field + 23, width);
        invalid[field + 15] = flags;
        invalid[field + 16] = extended;
        assert!(matches!(
            qualify(&schema(&invalid), &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
}
