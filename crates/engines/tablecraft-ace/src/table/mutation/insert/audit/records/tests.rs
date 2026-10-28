//! Literal record bytes distinguish carriers, physical spans and preferred wires.
use super::*;
use crate::{inspect_table_schema, test_support::base};
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as literal;

const PAGE: usize = 4096;
const TABLE: usize = 10 * PAGE;
const FIELDS: usize = 63;
const FIELD_BYTES: usize = 25;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn source(fields: &[(&str, u8, u16, u16)], variables: &[(usize, u16, bool)]) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(fields, 0, 11 << 8);
    literal::put16(&mut definition, 43, variables.len() as u16);
    for (position, ordinal, compressed) in variables {
        let at = FIELDS + position * FIELD_BYTES;
        definition[at + 15] = 2;
        definition[at + 16] = u8::from(*compressed);
        literal::put16(&mut definition, at + 7, *ordinal);
    }
    bytes[TABLE..TABLE + PAGE].copy_from_slice(&definition);
    bytes
}

fn schema(bytes: &[u8]) -> TableSchema {
    inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn reconstruct(
    bytes: &[u8],
    schema: &TableSchema,
    id: usize,
    values: Vec<Value>,
) -> Result<Vec<u8>, EditError> {
    let mut cursor = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut cursor,
        header: schema.header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let rows = derive(schema, id, &[RowInsert::new(values)], None, &mut reader)?;
    assert_eq!(rows.len(), 1);
    Ok(rows.into_iter().next().unwrap().bytes)
}

#[test]
fn later_primary_all_fixed_carriers_and_boolean_masks_have_literal_bytes() {
    let fields = [
        ("Companion", 4, 0, 4),
        ("ActualKey", 4, 4, 4),
        ("Switch", 1, 3, 0),
        ("Small", 2, 8, 1),
        ("Short", 3, 9, 2),
        ("Wide", 19, 11, 8),
        ("Money", 5, 19, 8),
        ("Float", 6, 27, 4),
        ("Double", 7, 31, 8),
        ("When", 8, 39, 8),
        ("Identity", 15, 47, 16),
    ];
    let bytes = source(&fields, &[]);
    let schema = schema(&bytes);
    qualify(&schema, &mut budget(), &|| false).unwrap();
    let values = vec![
        Value::Null,
        Value::Int32(1234),
        Value::Boolean(false),
        Value::Byte(254),
        Value::Int16(-7),
        Value::Int64(-123456789),
        Value::Currency(Currency::from_scaled(-12345)),
        Value::Single(Single::from_bits(0x80000000)),
        Value::Double(Double::from_bits(0x7ff8000000000042)),
        Value::DateTime(LegacyDateTime::from_ole_days(-2.25).unwrap()),
        Value::Guid(Guid::from_bytes([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ])),
    ];
    let mut expected = vec![0; 67];
    expected[..2].copy_from_slice(&[11, 0]);
    expected[6..10].copy_from_slice(&[0xd2, 4, 0, 0]);
    expected[10] = 254;
    expected[11..13].copy_from_slice(&(-7_i16).to_le_bytes());
    expected[13..21].copy_from_slice(&(-123456789_i64).to_le_bytes());
    expected[21..29].copy_from_slice(&(-12345_i64).to_le_bytes());
    expected[29..33].copy_from_slice(&[0, 0, 0, 0x80]);
    expected[33..41].copy_from_slice(&[0x42, 0, 0, 0, 0, 0, 0xf8, 0x7f]);
    expected[41..49].copy_from_slice(&(-2.25_f64).to_le_bytes());
    expected[49..65].copy_from_slice(&[3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15]);
    expected[65..].copy_from_slice(&[0xfa, 7]);
    assert_eq!(
        reconstruct(&bytes, &schema, 1, values.clone()).unwrap(),
        expected
    );

    let mut true_values = values;
    true_values[2] = Value::Boolean(true);
    expected[65] = 0xfe;
    assert_eq!(
        reconstruct(&bytes, &schema, 1, true_values).unwrap(),
        expected
    );
    let mut nulls = vec![Value::Null; 11];
    nulls[1] = Value::Int32(1234);
    nulls[2] = Value::Boolean(false);
    let mut empty = vec![0; 67];
    empty[..2].copy_from_slice(&[11, 0]);
    empty[6..10].copy_from_slice(&[0xd2, 4, 0, 0]);
    empty[65] = 2;
    assert_eq!(
        reconstruct(&bytes, &schema, 1, nulls.clone()).unwrap(),
        empty
    );
    nulls[2] = Value::Null;
    assert!(matches!(
        reconstruct(&bytes, &schema, 1, nulls),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn variable_ordinals_reconstruct_reverse_footer_and_distinct_null_empty_masks() {
    let fields = [
        ("LastBinary", 9, 0, 255),
        ("ActualKey", 4, 0, 4),
        ("FirstText", 10, 0, 510),
        ("Switch", 1, 1, 0),
        ("MiddleBinary", 9, 0, 255),
        ("LastText", 10, 0, 510),
    ];
    let bytes = source(
        &fields,
        &[(0, 2, false), (2, 0, true), (4, 1, false), (5, 3, false)],
    );
    let schema = schema(&bytes);
    let mut values = vec![
        Value::Binary(vec![0xaa, 0xbb]),
        Value::Int32(10),
        Value::Text("ABC".into()),
        Value::Boolean(false),
        Value::Null,
        Value::Text("AB".into()),
    ];
    let mut expected = vec![
        6, 0, 10, 0, 0, 0, 0xff, 0xfe, 65, 66, 67, 0xaa, 0xbb, 65, 0, 66, 0,
    ];
    expected.extend([17, 0, 13, 0, 11, 0, 11, 0, 6, 0, 4, 0, 0x27]);
    assert_eq!(
        reconstruct(&bytes, &schema, 1, values.clone()).unwrap(),
        expected
    );
    values[4] = Value::Binary(Vec::new());
    *expected.last_mut().unwrap() = 0x37;
    assert_eq!(reconstruct(&bytes, &schema, 1, values).unwrap(), expected);
}

#[test]
fn preferred_text_wires_reject_alternate_equivalent_encodings_and_bom_alias() {
    let bytes = source(&[("ID", 4, 0, 4), ("Text", 10, 0, 510)], &[(1, 0, true)]);
    let schema = schema(&bytes);
    for (text, expected) in [
        ("", vec![]),
        ("AB", vec![65, 0, 66, 0]),
        ("ABC", vec![0xff, 0xfe, 65, 66, 67]),
        ("A\0B", vec![65, 0, 0, 0, 66, 0]),
        ("\u{1f600}", vec![0x3d, 0xd8, 0, 0xde]),
    ] {
        let actual = payload::encode(
            &schema.columns()[1],
            &Value::Text(text.into()),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
    assert_eq!(
        crate::format::text::decode(&[0xff, 0xfe, 65, 66], true, &mut budget()).unwrap(),
        "AB"
    );
    assert_ne!(
        payload::encode(
            &schema.columns()[1],
            &Value::Text("AB".into()),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        [0xff, 0xfe, 65, 66]
    );
    assert!(matches!(
        payload::encode(
            &schema.columns()[1],
            &Value::Text("\u{feff}ABC".into()),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn boolean_offsets_cannot_invent_payload_extent_and_invalid_layouts_refuse() {
    let fields = [("ID", 4, 0, 4), ("Switch", 1, 3, 0)];
    let bytes = source(&fields, &[]);
    let metadata = schema(&bytes);
    qualify(&metadata, &mut budget(), &|| false).unwrap();
    assert_eq!(
        reconstruct(
            &bytes,
            &metadata,
            0,
            vec![Value::Int32(7), Value::Boolean(true)]
        )
        .unwrap(),
        [2, 0, 7, 0, 0, 0, 3]
    );

    let mut beyond = source(&fields, &[]);
    literal::put16(&mut beyond, TABLE + FIELDS + FIELD_BYTES + 21, 100);
    assert!(matches!(
        qualify(&schema(&beyond), &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    let mut overlap = source(&[("ID", 4, 0, 4), ("Other", 4, 2, 4)], &[]);
    assert!(matches!(
        qualify(&schema(&overlap), &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    literal::put16(&mut overlap, TABLE + FIELDS + FIELD_BYTES + 21, 4);
    qualify(&schema(&overlap), &mut budget(), &|| false).unwrap();
    let extended = source(&[("ID", 4, 0, 4), ("Extended", 20, 4, 42)], &[]);
    let metadata = schema(&extended);
    assert!(metadata.columns()[1].scalar_type().is_some());
    assert!(qualify(&metadata, &mut budget(), &|| false).is_err());

    for kind in [12, 11] {
        let ordinary = source(
            &[("ID", 4, 0, 4), ("LongValue", kind, 0, 0)],
            &[(1, 0, false)],
        );
        qualify(&schema(&ordinary), &mut budget(), &|| false).unwrap();
        for (flags, extended, declared) in [(2, 16, 0), (6, 0, 0), (2, 0, 1)] {
            let mut unsupported = ordinary.clone();
            let field = TABLE + FIELDS + FIELD_BYTES;
            unsupported[field + 15] = flags;
            unsupported[field + 16] = extended;
            literal::put16(&mut unsupported, field + 23, declared);
            assert!(qualify(&schema(&unsupported), &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
fn decimal_reconstruction_uses_literal_words_and_rejects_scale_precision_and_carrier_changes() {
    let mut bytes = source(&[("ID", 4, 0, 4), ("Amount", 16, 0, 17)], &[(1, 0, false)]);
    bytes[TABLE + FIELDS + FIELD_BYTES + 11] = 28;
    bytes[TABLE + FIELDS + FIELD_BYTES + 12] = 4;
    let metadata = schema(&bytes);
    qualify(&metadata, &mut budget(), &|| false).unwrap();

    let mut expected = vec![2, 0, 7, 0, 0, 0];
    expected.extend_from_slice(&[128, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9]);
    expected.extend_from_slice(&[23, 0, 6, 0, 1, 0, 3]);
    assert_eq!(
        reconstruct(
            &bytes,
            &metadata,
            0,
            vec![
                Value::Int32(7),
                Value::Decimal(Decimal::new(-311917102708983781990730508, 4).unwrap()),
            ],
        )
        .unwrap(),
        expected,
    );
    for value in [
        Value::Int32(1),
        Value::Decimal(Decimal::new(1, 3).unwrap()),
        Value::Decimal(Decimal::new(10_i128.pow(28), 4).unwrap()),
    ] {
        assert_eq!(
            reconstruct(&bytes, &metadata, 0, vec![Value::Int32(7), value]),
            Err(EditError::Unsupported(WriteBlocker::Column)),
        );
    }
}

#[test]
fn inline_memo_and_ole_descriptors_keep_null_empty_and_compressed_payload_separate() {
    let before = source(
        &[("ID", 4, 0, 4), ("Object", 11, 0, 0), ("Memo", 12, 0, 0)],
        &[(1, 0, false), (2, 1, true)],
    );
    let metadata = schema(&before);
    let encoded = reconstruct(
        &before,
        &metadata,
        0,
        vec![
            Value::Int32(7),
            Value::Binary(Vec::new()),
            Value::Text("ABC".into()),
        ],
    )
    .unwrap();
    let mut expected = vec![3, 0, 7, 0, 0, 0];
    expected.extend([0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0]);
    expected.extend([5, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 255, 254, 65, 66, 67]);
    expected.extend([35, 0, 18, 0, 6, 0, 2, 0, 7]);
    assert_eq!(encoded, expected);

    let null = reconstruct(
        &before,
        &metadata,
        0,
        vec![Value::Int32(7), Value::Null, Value::Text(String::new())],
    )
    .unwrap();
    let mut expected = vec![3, 0, 7, 0, 0, 0];
    expected.extend([0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0]);
    expected.extend([18, 0, 6, 0, 6, 0, 2, 0, 5]);
    assert_eq!(null, expected);
}

#[test]
fn memo_inline_encoding_retains_raw_ties_and_refuses_nonroundtripping_bom_or_external_choice() {
    let before = source(&[("ID", 4, 0, 4), ("Memo", 12, 0, 0)], &[(1, 0, true)]);
    let metadata = schema(&before);
    let row = reconstruct(
        &before,
        &metadata,
        0,
        vec![Value::Int32(1), Value::Text("AB".into())],
    )
    .unwrap();
    assert_eq!(
        &row[6..22],
        &[4, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 65, 0, 66, 0]
    );
    assert_eq!(&row[22..], &[22, 0, 6, 0, 1, 0, 3]);
    for value in [Value::Text("\u{feff}A".into()), Value::Text("A".repeat(63))] {
        assert!(reconstruct(&before, &metadata, 0, vec![Value::Int32(1), value]).is_err());
    }
    let wrong = reconstruct(
        &before,
        &metadata,
        0,
        vec![Value::Int32(1), Value::Binary(Vec::new())],
    );
    assert!(matches!(
        wrong,
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}
