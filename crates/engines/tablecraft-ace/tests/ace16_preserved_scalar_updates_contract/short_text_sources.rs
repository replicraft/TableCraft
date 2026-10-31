//! Inert rich originals and literal fitting Short Text expectations before writer.
#[path = "short_text_sources/api.rs"]
mod api;
#[path = "short_text_sources/model.rs"]
mod model;
#[path = "short_text_sources/zero_length_source.rs"]
pub(super) mod zero_length_source;

use super::{SCOPE, budget, model as rich};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, inspect_calculation_declaration,
    inspect_table_indexes, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation, read_table_index_projection, read_table_projection,
};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

fn qualify(bytes: &[u8], code: u8, row: usize, value: Option<&str>) {
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let field = &schema.columns()[1];
    assert_eq!(field.name(), "Short_Text");
    assert_eq!(
        (field.native_type(), field.number(), field.storage_length()),
        (10, 1, 510)
    );
    assert_eq!(field.scalar_type(), Some(ScalarType::Text));
    assert!(!field.fixed());
    for number in [7, 8] {
        assert_eq!(schema.columns()[number].native_type(), 20);
        assert_eq!(
            schema.columns()[number].scalar_type(),
            if code == 6 {
                Some(ScalarType::DateTimeExtended)
            } else {
                None
            }
        );
    }
    let table = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &["ID", "Short_Text"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut expected = vec![
        vec![Value::Int32(1), Value::Text("alpha".to_owned())],
        vec![Value::Int32(2), Value::Null],
        vec![Value::Int32(3), Value::Text("omega".to_owned())],
    ];
    expected[row][1] = value.map_or(Value::Null, |text| Value::Text(text.to_owned()));
    assert_eq!(table.rows(), expected);
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
            .all(|index| index.columns().iter().all(|column| column.number() != 1))
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
    let stored = read_stored_int32_calculation(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Calculated",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(stored.values(), &[Some(2), Some(4), Some(6)]);
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
}

#[test]
fn qualify_preserved_short_text_originals_and_literal_fitting_images_before_writer() {
    let mut expected_images = 0;
    let mut overflow_controls = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            assert_eq!(case.admitted_source, code == 6);
            assert_eq!(case.framing.present, case.original.is_some());
            assert_eq!(case.framing.count, 16);
            qualify(&case.before, code, case.framing.row, case.original);
            if let Some(expected) = &case.expected {
                assert_eq!(expected.len(), case.before.len());
                qualify(expected, code, case.framing.row, case.replacement);
                if matches!(recipe, model::Recipe::Noop | model::Recipe::NullNoop) {
                    assert_eq!(expected, &case.before);
                }
                assert!(case.payload.unwrap().len() <= case.framing.capacity);
                expected_images += 1;
            } else {
                assert_eq!(recipe, model::Recipe::Overflow);
                assert_eq!(case.replacement.unwrap().encode_utf16().count() * 2, 12);
                assert_eq!(case.framing.capacity, 10);
                overflow_controls += 1;
            }
        }
    }
    assert_eq!((expected_images, overflow_controls), (16, 2));
}
