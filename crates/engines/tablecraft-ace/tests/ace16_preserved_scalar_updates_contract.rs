//! Source-first qualification for a donor-shaped preservation-only Boolean edit.
#[path = "ace16_preserved_scalar_updates_contract/api.rs"]
mod api;
#[path = "ace16_preserved_scalar_updates_contract/asymmetric_guids.rs"]
mod asymmetric_guids;
#[path = "ace16_preserved_scalar_updates_contract/authority_sources.rs"]
mod authority_sources;
use tablecraft_testkit::access::mutation_fixture as base;
#[path = "ace16_preserved_scalar_updates_contract/catalog_flag.rs"]
mod catalog_flag;
#[path = "complex_schema/fixture.rs"]
pub mod complex_fixture;
#[path = "ace16_preserved_scalar_updates_contract/growing_sources.rs"]
mod growing_sources;
#[path = "ace16_preserved_scalar_updates_contract/model.rs"]
mod model;
#[path = "attachment_modern/fixture.rs"]
pub mod modern_attachment;
#[path = "ace16_preserved_scalar_updates_contract/name_map.rs"]
mod name_map;
#[path = "ace16_preserved_scalar_updates_contract/name_map_controls.rs"]
mod name_map_controls;
#[path = "attachments/fixture.rs"]
pub mod old_attachment;
#[path = "ace16_preserved_scalar_updates_contract/outer_url.rs"]
mod outer_url;

use std::io::Cursor;
use support as raw;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, LongValueCheckStatus, check_table,
    check_table_long_values, inspect_calculation_declaration, inspect_table_indexes,
    inspect_table_properties, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation, read_table, read_table_index_projection, read_table_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn qualify_preserved_boolean_16_field_source_and_one_bit_expected_before_writer() {
    let case = model::source();
    assert_eq!(case.before.len(), 32 * 4096);
    assert_eq!(case.before.len(), case.expected.len());
    let differences = case
        .before
        .iter()
        .zip(&case.expected)
        .enumerate()
        .filter_map(|(offset, (before, after))| {
            (before != after).then_some((offset, before ^ after))
        })
        .collect::<Vec<_>>();
    assert_eq!(differences, [(model::BOOLEAN_PATCH_OFFSET, 4)]);

    for (bytes, first) in [(&case.before, false), (&case.expected, true)] {
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
        for (number, kind, width, flags, extended) in [
            (0, 4, 4, 7, 0),
            (1, 10, 510, 2, 1),
            (2, 12, 0, 2, 0),
            (7, 20, 42, 3, 0),
            (8, 20, 42, 3, 0),
            (10, 1, 0, 3, 0),
            (11, 1, 0, 3, 0),
            (12, 11, 0, 2, 0),
            (13, 12, 0, 0x82, 0),
            (14, 18, 4, 7, 0),
            (15, 4, 39, 2, 0xc0),
        ] {
            let field = &schema.columns()[number];
            assert_eq!(
                (
                    field.native_type(),
                    field.storage_length(),
                    field.flags(),
                    field.extended_flags()
                ),
                (kind, width, flags, extended)
            );
        }
        assert!(schema.scalar_columns().is_err());
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
        assert_eq!(
            check_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );

        let selected = read_table_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &["ID", "Yes_Field", "No_Field"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let ordinary = read_table_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &[
                "ID",
                "Short_Text",
                "Long_Text",
                "Number",
                "Large_Number",
                "Date_Field",
                "Time_Field",
                "Date_Extended_Field",
                "Time_Extended_Field",
                "Currency_Field",
                "Yes_Field",
                "No_Field",
                "OLE_Object_Field",
                "Hyperlink_Field",
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(ordinary.columns().len(), 14);
        assert_eq!(ordinary.rows().len(), 3);
        assert_eq!(ordinary.rows()[0][4], Value::Int64(1000));
        assert_eq!(ordinary.rows()[1][7], Value::Null);
        match &ordinary.rows()[0][7] {
            Value::DateTimeExtended(value) => {
                assert_eq!(value.date(), (1, 1, 1));
                assert_eq!(value.time(), (0, 0, 0, 0));
            }
            _ => panic!("missing genuine extended timestamp"),
        }
        match &ordinary.rows()[0][8] {
            Value::DateTimeExtended(value) => assert_eq!(value.time(), (0, 0, 0, 1)),
            _ => panic!("missing genuine extended tick"),
        }
        match &ordinary.rows()[0][9] {
            Value::Currency(value) => assert_eq!(value.scaled(), 12345),
            _ => panic!("missing genuine currency"),
        }

        assert_eq!(
            selected.rows(),
            &[
                vec![
                    Value::Int32(1),
                    Value::Boolean(first),
                    Value::Boolean(false)
                ],
                vec![Value::Int32(2), Value::Boolean(false), Value::Boolean(true)],
                vec![
                    Value::Int32(3),
                    Value::Boolean(false),
                    Value::Boolean(false)
                ],
            ]
        );
    }
}

#[test]
fn qualify_preserved_boolean_nonempty_attachment_calculation_and_lval_sources() {
    let case = model::source();
    for bytes in [&case.before, &case.expected] {
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
        let (child_start, child_end) = modern_attachment::span(bytes, 0);
        assert!(child_end > child_start);

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
            &[
                vec![
                    Value::Text("alpha".to_owned()),
                    Value::Text("memo".to_owned()),
                    Value::Binary(vec![0, 255, 7]),
                    Value::Text("label#https://invalid.test/#".to_owned())
                ],
                vec![Value::Null, Value::Null, Value::Null, Value::Null],
                vec![
                    Value::Text("omega".to_owned()),
                    Value::Text("tail".to_owned()),
                    Value::Binary(Vec::new()),
                    Value::Text("synthetic#relative#".to_owned())
                ],
            ]
        );
        let storage = check_table_long_values(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(storage.columns().len(), 3);
        for column in storage.columns() {
            match column.column_number() {
                2 | 12 => {
                    assert_eq!(column.status(), LongValueCheckStatus::Checked);
                    assert_eq!(
                        (
                            column.values().nulls(),
                            column.values().inline(),
                            column.values().single()
                        ),
                        (1, 1, 1)
                    );
                }
                13 => assert_eq!(column.status(), LongValueCheckStatus::UnsupportedColumn),
                _ => panic!("unexpected LVAL column"),
            }
        }
    }
}

#[test]
fn qualify_preserved_boolean_properties_and_three_original_index_owners() {
    let case = model::source();
    for bytes in [&case.before, &case.expected] {
        let properties = inspect_table_properties(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(properties.groups().len(), 17);
        let indexes = inspect_table_indexes(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexes.indexes().len(), 3);
        for name in ["NumberKey", "PrimaryKey"] {
            let values = read_table_index_projection(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                name,
                IndexProjection {
                    columns: &["ID"],
                    selection: IndexProjectionSelection::All,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                values.rows(),
                &[
                    vec![Value::Int32(1)],
                    vec![Value::Int32(2)],
                    vec![Value::Int32(3)]
                ]
            );
        }
        // The complex reference is not exposed as an ordinary SQL Int32. Its
        // original key membership is cross-checked against the specific reader.
        let attachments = read_attachment_column(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            "Attachment_Field",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let leaf = &bytes[27 * 4096..28 * 4096];
        for (ordinal, parent) in attachments.links().parents().iter().enumerate() {
            let key = parent.key().unwrap();
            let at = 480 + ordinal * 9;
            assert_eq!(leaf[at], 0x7f);
            assert_eq!(
                &leaf[at + 1..at + 5],
                &((key as u32) ^ 0x8000_0000).to_be_bytes()
            );
            assert_eq!(
                &leaf[at + 5..at + 9],
                &((12_u32 << 8) | ordinal as u32).to_be_bytes()
            );
        }
    }
}

#[path = "ace16_preserved_scalar_updates_contract/int32_sources.rs"]
mod int32_sources;

#[path = "ace16_preserved_scalar_updates_contract/int32_sources/api.rs"]
mod int32_api;

#[path = "ace16_preserved_scalar_updates_contract/short_text_sources.rs"]
mod short_text_sources;
