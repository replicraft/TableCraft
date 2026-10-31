//! Read-only source observations, with public API acceptance in a separate child.
#[path = "fixed_scalar_sources/model.rs"]
pub(crate) mod model;

#[path = "fixed_scalar_sources/api.rs"]
mod api;

use super::{SCOPE, budget, model as rich};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, inspect_table_indexes, inspect_table_schema,
    read_attachment_column, read_stored_int32_calculation, read_table_index_projection,
    read_table_projection,
};
use tablecraft_core::values::Value;

fn qualify(bytes: &[u8], case: &model::Case, values: [Option<[u8; 8]>; 3]) -> Vec<Vec<Value>> {
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
    let field = &schema.columns()[case.target.number()];
    assert_eq!(field.name(), case.target.name());
    assert_eq!(field.native_type(), case.target.native());
    assert_eq!(field.number(), case.target.number());
    assert_eq!(field.storage_length(), 8);
    assert_eq!((field.flags(), field.extended_flags()), (3, 0));
    assert!(field.fixed());

    let names = schema
        .columns()
        .iter()
        .filter(|field| field.scalar_type().is_some())
        .map(|field| field.name())
        .collect::<Vec<_>>();
    let projected = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &names,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected = projected
        .columns()
        .iter()
        .position(|field| field.name() == case.target.name())
        .unwrap();
    let id = projected
        .columns()
        .iter()
        .position(|field| field.name() == "ID")
        .unwrap();
    for (ordinal, row) in projected.rows().iter().enumerate() {
        assert_eq!(row[id], Value::Int32((ordinal + 1) as i32));
        assert_eq!(row[selected], case.target.value(values[ordinal]));
    }

    let indexes = inspect_table_indexes(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexes.indexes().len(), 3);
    assert!(indexes.indexes().iter().all(|index| {
        index
            .columns()
            .iter()
            .all(|key| key.number() != case.target.number())
    }));
    for name in ["NumberKey", "PrimaryKey"] {
        let index_rows = read_table_index_projection(
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
            index_rows.rows(),
            [
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)]
            ]
        );
    }
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
    projected.into_parts().2
}

#[test]
fn qualify_preserved_eight_byte_sources_and_literal_images_before_writer() {
    let mut originals = 0;
    let mut candidates = 0;
    let mut required = 0;
    for code in [5, 6] {
        for (target, recipe) in model::cases() {
            let case = model::source(code, target, recipe);
            model::assert_complement(&case);
            let before = qualify(&case.before, &case, case.initial);
            originals += 1;
            if let Some(expected) = &case.expected {
                let after = qualify(expected, &case, case.replacement);
                let schema = inspect_table_schema(
                    &mut Cursor::new(expected),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let ordinary = schema
                    .columns()
                    .iter()
                    .filter(|field| field.scalar_type().is_some())
                    .map(|field| field.name())
                    .collect::<Vec<_>>();
                for (old, new) in before.iter().zip(after) {
                    for ((old, new), name) in old.iter().zip(new).zip(&ordinary) {
                        if *name != target.name() {
                            assert_eq!(old, &new);
                        }
                    }
                }
                candidates += 1;
            } else {
                assert_eq!(recipe, model::Recipe::RequiredRefusal);
                assert_eq!(case.before[case.required_patch.unwrap()], 1);
                assert!(case.initial.iter().all(Option::is_some));
                required += 1;
            }
        }
    }
    assert_eq!((originals, candidates, required), (78, 70, 8));
}
