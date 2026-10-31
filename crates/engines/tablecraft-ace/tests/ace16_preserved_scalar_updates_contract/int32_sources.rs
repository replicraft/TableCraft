//! Qualified authored rich originals and exact four-byte expected images.
#[path = "int32_sources/model.rs"]
pub(super) mod model;

use super::{SCOPE, budget, model as rich, old_attachment};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, inspect_calculation_declaration,
    inspect_table_indexes, inspect_table_properties, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation, read_table, read_table_index_projection, read_table_projection,
};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

const PAGE_BYTES: usize = 4096;
const ORDINARY_OWNED: [(usize, &[usize]); 6] = [
    (3, &[4]),
    (8, &[]),
    (11, &[12]),
    (15, &[16]),
    (18, &[]),
    (21, &[22]),
];

fn map(bytes: &[u8], page: usize, slot: usize) -> &[u8] {
    let data = &bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES];
    assert_eq!(&data[..2], &[1, 1]);
    assert_eq!(
        &data[4..8],
        &(if page == 1 { 1_u32 } else { 0_u32 }).to_le_bytes()
    );
    let count = usize::from(u16::from_le_bytes(data[12..14].try_into().unwrap()));
    assert!(slot < count);
    let at = 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(data[at..at + 2].try_into().unwrap()));
    let end = if slot == 0 {
        PAGE_BYTES
    } else {
        usize::from(u16::from_le_bytes(data[at - 2..at].try_into().unwrap()))
    };
    assert_eq!(start & 0xe000, 0);
    assert!(start < end);
    let row = &data[start..end];
    assert_eq!(&row[..5], &[0; 5]);
    row
}

fn members(row: &[u8]) -> Vec<usize> {
    row[5..]
        .iter()
        .enumerate()
        .flat_map(|(byte, mask)| {
            (0..8).filter_map(move |bit| (mask & (1 << bit) != 0).then_some(byte * 8 + bit))
        })
        .collect()
}

fn roles(bytes: &[u8]) {
    assert_eq!(bytes.len(), 32 * PAGE_BYTES);
    let global = map(bytes, 1, 0);
    assert_eq!(members(global), (32..512).collect::<Vec<_>>());
    for page in 0..32 {
        assert_eq!(global[5 + page / 8] & (1 << (page % 8)), 0);
    }
    for (container, owned) in ORDINARY_OWNED {
        assert_eq!(members(map(bytes, container, 0)), owned);
        assert_eq!(members(map(bytes, container, 1)), owned);
    }
    for (owned, candidate, page) in [(2, 3, 24), (4, 5, 25), (6, 7, 26)] {
        assert_eq!(members(map(bytes, 11, owned)), [page]);
        assert_eq!(members(map(bytes, 11, candidate)), [page]);
    }
    for physical in 0..3 {
        assert_eq!(members(map(bytes, 30, physical)), [27 + physical]);
    }
    for (page, owner) in [
        (4, 2_u32),
        (12, 10),
        (16, 14),
        (22, 20),
        (27, 10),
        (28, 10),
        (29, 10),
    ] {
        assert_eq!(
            &bytes[page * PAGE_BYTES + 4..page * PAGE_BYTES + 8],
            &owner.to_le_bytes()
        );
    }
    for page in [24, 25, 26] {
        assert_eq!(
            &bytes[page * PAGE_BYTES + 4..page * PAGE_BYTES + 8],
            b"LVAL"
        );
    }
}

fn qualify(bytes: &[u8], first: i32) {
    roles(bytes);
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 16);
    assert_eq!(schema.declared_rows(), 3);
    // Readable selected columns do not imply complete preservation admission.
    // The unchanged rich source carries Extended Date declarations: code6 has
    // scalar coverage; code5 must remain a preservation-refusal control.
    for position in [7, 8] {
        let field = &schema.columns()[position];
        assert_eq!(
            (
                field.native_type(),
                field.storage_length(),
                field.flags(),
                field.extended_flags()
            ),
            (20, 42, 3, 0)
        );
        assert_eq!(
            field.scalar_type(),
            if bytes[20] == 6 {
                Some(ScalarType::DateTimeExtended)
            } else {
                None
            }
        );
    }

    let target = &schema.columns()[4];
    assert_eq!(target.name(), "Large_Number");
    assert_eq!(
        (
            target.native_type(),
            target.storage_length(),
            target.flags(),
            target.extended_flags()
        ),
        (4, 4, 3, 0)
    );
    assert_eq!(
        read_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Unsupported)
    );
    let projected = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &["ID", "Number", "Large_Number", "Yes_Field"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        projected.rows(),
        [
            vec![
                Value::Int32(1),
                Value::Int32(1),
                Value::Int32(first),
                Value::Boolean(false)
            ],
            vec![
                Value::Int32(2),
                Value::Int32(2),
                Value::Int32(2000),
                Value::Boolean(false)
            ],
            vec![
                Value::Int32(3),
                Value::Int32(3),
                Value::Int32(3000),
                Value::Boolean(false)
            ],
        ]
    );
    let indexes = inspect_table_indexes(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexes.indexes().len(), 3);
    assert!(
        indexes
            .indexes()
            .iter()
            .all(|index| index.columns().iter().all(|column| column.number() != 4))
    );
    for index in ["NumberKey", "PrimaryKey"] {
        let selected = read_table_index_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            index,
            IndexProjection {
                columns: &["ID"],
                selection: IndexProjectionSelection::All,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            selected.rows(),
            [
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)]
            ]
        );
    }
    let declaration = inspect_calculation_declaration(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Calculated",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(declaration.expression().unwrap(), "[ID]+[Number]");
    let cached = read_stored_int32_calculation(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Calculated",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(cached.values(), &[Some(2), Some(4), Some(6)]);
    let properties = inspect_table_properties(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.groups().len(), 17);
    let attachments = read_attachment_column(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Attachment_Field",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        attachments
            .links()
            .parents()
            .iter()
            .map(|parent| (parent.key(), parent.child_count()))
            .collect::<Vec<_>>(),
        [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
    );
    assert_eq!(
        attachments.stored_value(0, 0, "FileData"),
        Some(&Value::Binary(old_attachment::PAYLOAD.to_vec()))
    );
    assert_eq!(
        attachments.stored_value(0, 1, "FileData"),
        Some(&Value::Null)
    );
    assert_eq!(
        attachments.stored_value(1, 0, "FileData"),
        Some(&Value::Binary(Vec::new()))
    );
    let payloads = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &[
            "Short_Text",
            "Long_Text",
            "OLE_Object_Field",
            "Hyperlink_Field",
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        payloads.rows(),
        [
            vec![
                Value::Text("alpha".into()),
                Value::Text("memo".into()),
                Value::Binary(vec![0, 255, 7]),
                Value::Text("label#https://invalid.test/#".into())
            ],
            vec![Value::Null, Value::Null, Value::Null, Value::Null],
            vec![
                Value::Text("omega".into()),
                Value::Text("tail".into()),
                Value::Binary(vec![]),
                Value::Text("synthetic#relative#".into())
            ],
        ]
    );
}

#[test]
fn qualify_preserved_int32_rich_sources_and_four_byte_expected_before_producer() {
    let mut pairs = 0;
    for (code, value) in model::CASES {
        let case = model::source(code, value);
        assert_eq!(case.value, value);
        assert_eq!(
            &case.before[case.patch..case.patch + 4],
            &1000_i32.to_le_bytes()
        );
        assert_eq!(
            &case.expected[case.patch..case.patch + 4],
            &value.to_le_bytes()
        );
        // Negative replacement must not sign-extend into the retained former
        // upper word, nor touch any other field, NULL mask or opaque byte.
        assert_eq!(&case.before[case.patch + 4..case.patch + 8], &[0; 4]);
        assert_eq!(&case.expected[case.patch + 4..case.patch + 8], &[0; 4]);
        assert_eq!(&case.before[..case.patch], &case.expected[..case.patch]);
        assert_eq!(
            &case.before[case.patch + 4..],
            &case.expected[case.patch + 4..]
        );
        qualify(&case.before, 1000);
        qualify(&case.expected, value);
        pairs += 1;
    }
    assert_eq!(pairs, 8);
}
