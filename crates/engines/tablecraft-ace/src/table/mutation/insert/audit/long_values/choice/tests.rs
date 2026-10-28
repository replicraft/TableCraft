//! Literal physical ordinals and complete row sizes determine externalization.
use super::*;
use crate::{
    inspect_table_schema,
    table::mutation::insert::{
        audit::{
            long_values::{Resolved, ResolvedRow},
            records,
        },
        record::{self, input},
    },
    test_support::base,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn source(padding_offset: u16) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(
        &[
            ("ID", 4, 0, 4),
            ("Padding", 4, padding_offset, 4),
            ("WrittenFirst", 11, 0, 0),
            ("PhysicalFirst", 11, 0, 0),
        ],
        0,
        11 << 8,
    );
    raw::put16(&mut definition, 43, 2);
    for (position, ordinal) in [(2, 1), (3, 0)] {
        let at = 63 + position * 25;
        definition[at + 15] = 2;
        raw::put16(&mut definition, at + 7, ordinal);
    }
    bytes[10 * 4096..11 * 4096].copy_from_slice(&definition);
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

fn choices(bytes: &[u8], values: Vec<Value>) -> Result<Vec<Row>, EditError> {
    let schema = schema(bytes);
    let mut cursor = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut cursor,
        header: schema.header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    derive(&schema, &[RowInsert::new(values)], &mut reader)
}

#[test]
fn physical_ordinal_first_sufficient_payload_is_externalized_with_literal_complete_bytes() {
    let bytes = source(3934);
    let values = vec![
        Value::Int32(0x01020304),
        Value::Int32(7),
        Value::Binary(vec![0x92; 64]),
        Value::Binary(vec![0x3d; 64]),
    ];
    let rows = choices(&bytes, values.clone()).unwrap();
    let row = &rows[0];
    assert_eq!(row.record_length, 4037);
    assert_eq!(row.fields.len(), 2);
    assert_eq!(row.fields[0].column, 3);
    assert!(row.fields[0].storage == Storage::Single);
    assert_eq!(row.fields[1].column, 2);
    assert!(row.fields[1].storage == Storage::Inline);

    let external = vec![64, 0, 0, 64, 0, 16, 0, 0, 0, 0, 0, 0];
    let mut inline = vec![64, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0];
    inline.extend([0x92; 64]);
    let resolved = [ResolvedRow {
        fields: vec![
            Resolved {
                column: 3,
                descriptor: external.clone(),
            },
            Resolved {
                column: 2,
                descriptor: inline.clone(),
            },
        ],
        record_length: 4037,
    }];
    let mut expected = vec![0; 4037];
    expected[..6].copy_from_slice(&[4, 0, 4, 3, 2, 1]);
    expected[3936..3940].copy_from_slice(&[7, 0, 0, 0]);
    expected[3940..3952].copy_from_slice(&external);
    expected[3952..4028].copy_from_slice(&inline);
    expected[4028..].copy_from_slice(&[188, 15, 112, 15, 100, 15, 2, 0, 15]);

    let schema = schema(&bytes);
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut cursor,
        header: schema.header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let derived = records::derive(
        &schema,
        0,
        &[RowInsert::new(values.clone())],
        Some(&resolved),
        &mut reader,
    )
    .unwrap();
    assert_eq!(derived[0].bytes, expected);

    // The constructor is a consumer under test, after literal size/bytes and
    // independent choices are fixed. It supplies no expected byte authority.
    let profile = record::Profile::bind(&schema, &mut budget(), &|| false).unwrap();
    assert_eq!(profile.variables(), &[3, 2]);
    let producer = input::prepare(
        &schema,
        profile.variables(),
        &values,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        &producer.payloads[0],
        input::Payload::Long {
            choice: crate::table::mutation::long_value::encode::StorageChoice::Single,
            ..
        }
    ));
    assert!(matches!(
        &producer.payloads[1],
        input::Payload::Long {
            choice: crate::table::mutation::long_value::encode::StorageChoice::Inline,
            ..
        }
    ));
    let actual = profile
        .build_prepared(
            &values,
            input::Input {
                payloads: vec![
                    input::Payload::Bytes(external),
                    input::Payload::Bytes(inline),
                ],
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn null_and_empty_physical_first_values_remain_inline_when_later_payload_must_move() {
    let bytes = source(4000);
    for (first, length, empty) in [
        (Value::Null, 4027, false),
        (Value::Binary(vec![]), 4039, true),
    ] {
        let rows = choices(
            &bytes,
            vec![
                Value::Int32(1),
                Value::Int32(7),
                Value::Binary(vec![0x72; 64]),
                first,
            ],
        )
        .unwrap();
        let row = &rows[0];
        assert_eq!(row.record_length, length);
        assert_eq!(row.fields[0].column, 3);
        assert!(row.fields[0].storage == Storage::Inline);
        match &row.fields[0].payload {
            Payload::Null => assert!(!empty),
            Payload::Present(bytes) => {
                assert!(empty);
                assert!(bytes.is_empty());
            }
        }
        assert_eq!(row.fields[1].column, 2);
        assert!(row.fields[1].storage == Storage::Single);
    }
}

#[test]
fn irreducible_record_size_refuses_after_every_nonempty_inline_choice_is_exhausted() {
    let bytes = source(4044);
    assert!(matches!(
        choices(
            &bytes,
            vec![
                Value::Int32(1),
                Value::Int32(7),
                Value::Binary(vec![0x72; 64]),
                Value::Binary(vec![0x3d; 64]),
            ]
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout)),
    ));
    // Even empty descriptors cannot move; their twelve-byte headers remain.
    assert!(matches!(
        choices(
            &bytes,
            vec![
                Value::Int32(1),
                Value::Int32(7),
                Value::Binary(vec![]),
                Value::Binary(vec![]),
            ]
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout)),
    ));
}
