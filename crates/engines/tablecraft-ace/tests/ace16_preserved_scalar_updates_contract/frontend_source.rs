//! Source-only ordinary projection and preserved child/cache qualification.
use super::{SCOPE, budget, catalog_flag, model};
use std::io::Cursor;
use tablecraft_ace::{
    inspect_table_schema, read_attachment_column, read_stored_int32_calculation,
    read_table_projection,
};
use tablecraft_core::values::Value;

#[test]
fn qualify_preserved_boolean_frontend_sources_before_actual_commands() {
    for case in [model::source(), catalog_flag::source()] {
        for (bytes, first) in [(&case.before, false), (&case.expected, true)] {
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
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
            assert_eq!(names.len(), 14);
            let projected = read_table_projection(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(projected.columns().len(), 14);
            assert_eq!(projected.rows()[0][10], Value::Boolean(first));
            let attachments = read_attachment_column(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Attachment_Field",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(attachments.links().parents()[0].child_count(), 2);
            let calculation = read_stored_int32_calculation(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Calculated",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(calculation.values(), &[Some(2), Some(4), Some(6)]);
        }
    }
}
