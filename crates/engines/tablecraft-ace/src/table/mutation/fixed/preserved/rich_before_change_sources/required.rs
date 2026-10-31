//! Additive healthy Required originals; no mutation expectation or writer.
use super::super::fixed_scalar_sources::model as scalar_model;
use super::model as sources;

#[path = "required/model.rs"]
pub(crate) mod model;

use crate::{
    NativeImage, PropertyGroupKind, inspect_table_indexes, inspect_table_properties,
    inspect_table_schema, read_table_projection,
    table::mutation::variable_split::metadata::Metadata,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_rich_before_change_required_originals_before_expected_or_writer() {
    let scope = CatalogScope::new(1);
    let mut originals = 0;
    for code in [5, 6] {
        for repairing in [true, false] {
            let case = model::source(code, repairing);
            assert_eq!(case.repairing, repairing);
            assert_eq!(case.source.before[case.patch], 1);
            assert_eq!(case.source.original[case.original_patch], 1);
            let image = NativeImage::capture(
                &mut Cursor::new(&case.source.before),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let schema =
                inspect_table_schema(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(schema.columns()[4].name(), "Large_Number");
            assert_eq!(schema.columns()[4].native_type(), 4);
            assert_eq!(schema.columns()[4].storage_length(), 4);
            let properties = inspect_table_properties(
                &mut image.reader(),
                scope,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                properties
                    .group(PropertyGroupKind::Column, "Large_Number")
                    .unwrap()
                    .get("Required")
                    .unwrap()
                    .boolean(),
                Ok(true)
            );
            let original_properties = inspect_table_properties(
                &mut Cursor::new(&case.source.original),
                scope,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(properties.raw_bytes(), original_properties.raw_bytes());
            let names = schema
                .columns()
                .iter()
                .filter(|field| field.scalar_type().is_some())
                .map(|field| field.name())
                .collect::<Vec<_>>();
            let rows = read_table_projection(
                &mut image.reader(),
                scope,
                "Items",
                &names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let selected = rows
                .columns()
                .iter()
                .position(|field| field.name() == "Large_Number")
                .unwrap();
            assert_eq!(
                rows.rows()
                    .iter()
                    .map(|row| &row[selected])
                    .collect::<Vec<_>>(),
                [
                    &Value::Int32(1000),
                    &Value::Int32(2000),
                    &Value::Int32(3000)
                ]
            );
            crate::constraints::check_scalar_projection(&rows, &properties, &mut budget(), &|| {
                false
            })
            .unwrap();
            let coverage = image
                .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                coverage.table_extra().raw(),
                Some(case.source.extra.as_slice())
            );
            assert!(coverage.children().is_empty());
            assert_eq!(coverage.body().presence(), crate::PayloadPresence::Absent);
            assert_eq!(coverage.module().presence(), crate::PayloadPresence::Absent);
            let indexes =
                inspect_table_indexes(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            let metadata = Metadata::capture_for_preserved_boolean(
                &image,
                &schema,
                indexes.indexes(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            metadata
                .require_source(&image, schema.definition_page())
                .unwrap();
            assert_eq!(image.as_bytes(), case.source.before);
            originals += 1;
        }
    }
    assert_eq!(originals, 4);
}
