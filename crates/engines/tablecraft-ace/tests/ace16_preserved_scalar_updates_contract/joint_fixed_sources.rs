//! Read-only qualification of literal joint requests and complete expected images.
#[path = "joint_fixed_sources/model.rs"]
pub(crate) mod model;

use super::{
    SCOPE, budget, fixed_scalar_sources::model as fixed_scalar_model,
    int32_sources::model as int32_model, model as rich,
};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, inspect_table_indexes, inspect_table_schema,
    read_attachment_column, read_stored_int32_calculation, read_table_index_projection,
    read_table_projection,
};
use tablecraft_core::values::Value;

fn qualify(bytes: &[u8], case: &model::Case) -> Vec<Vec<Value>> {
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
    for column in model::columns(case.flavor) {
        let field = &schema.columns()[column.number()];
        assert_eq!(field.name(), column.name());
        assert_eq!(field.native_type(), column.native());
        assert_eq!(field.storage_length(), column.width());
        assert_eq!((field.flags(), field.extended_flags()), (3, 0));
        assert!(field.fixed());
    }
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
    assert_eq!(projected.rows().len(), 3);
    for column in model::columns(case.flavor) {
        let selected = projected
            .columns()
            .iter()
            .position(|field| field.name() == column.name())
            .unwrap();
        for (ordinal, row) in projected.rows().iter().enumerate() {
            assert_eq!(row[selected], model::read(bytes, column, ordinal).value());
        }
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
    for column in model::columns(case.flavor) {
        let indexed = indexes.indexes().iter().any(|index| {
            index
                .columns()
                .iter()
                .any(|key| key.number() == column.number())
        });
        assert_eq!(indexed, column == model::Column::IndexedNumber);
    }
    for name in ["NumberKey", "PrimaryKey"] {
        let keys = read_table_index_projection(
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
            keys.rows(),
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
fn qualify_joint_fixed_originals_and_literal_images_before_any_writer() {
    let mut originals = 0;
    let mut expected_images = 0;
    let mut refusal_originals = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            model::assert_complement(&case);
            let before = qualify(&case.before, &case);
            originals += 1;
            if let Some(expected) = &case.expected {
                let after = qualify(expected, &case);
                let schema = inspect_table_schema(
                    &mut Cursor::new(expected),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let names = schema
                    .columns()
                    .iter()
                    .filter(|field| field.scalar_type().is_some())
                    .map(|field| field.name())
                    .collect::<Vec<_>>();
                for (old, new) in before.iter().zip(after) {
                    for ((old, new), name) in old.iter().zip(new).zip(&names) {
                        if !case.groups.iter().any(|group| group.column.name() == *name) {
                            assert_eq!(old, &new);
                        }
                    }
                }
                expected_images += 1;
            } else {
                assert!(case.refusal.is_some());
                refusal_originals += 1;
            }
            if let Some(patch) = case.required_patch {
                assert_eq!(case.before[patch], 1);
                for row in 0..3 {
                    assert!(matches!(
                        model::read(&case.before, model::Column::Currency, row),
                        model::Cell::Currency(_)
                    ));
                }
            }
        }
    }
    assert_eq!(
        (originals, expected_images, refusal_originals),
        (54, 38, 16)
    );
}
