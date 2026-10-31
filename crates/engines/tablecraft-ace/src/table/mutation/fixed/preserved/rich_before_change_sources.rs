//! Inert rich catalog, program and ownership observations before batch writers.
use super::{int32::tests::model as int32_model, larger_capture_tests::preserved_model as rich};
use crate::preservation::table_extra::tests::multievent_sources as programs;
use tablecraft_testkit::access as raw;
#[path = "rich_before_change_sources/model.rs"]
pub(crate) mod model;

use crate::{
    NativeImage, PayloadPresence, TableExtraCoverage, inspect_table_indexes,
    inspect_table_properties, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation, read_table_projection,
    table::mutation::variable_split::metadata::Metadata,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_rich_before_change_catalog_roles_and_originals_before_batch_writer() {
    let mut sources = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let schema =
                inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(schema.columns().len(), 16);
            assert_eq!(
                schema.columns()[4].native_type(),
                if case.int32 { 4 } else { 19 }
            );
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
            if matches!(
                recipe,
                model::Recipe::CounterRead | model::Recipe::CounterTarget
            ) {
                assert_eq!(schema.columns()[0].name(), "ID");
                assert_eq!(schema.columns()[0].flags(), 7);
                assert_eq!(schema.columns()[0].extended_flags(), 0);
                assert_eq!(schema.columns()[0].storage_length(), 4);
                assert_eq!(schema.columns()[0].scalar_type(), Some(ScalarType::Int32));
            }
            let catalog = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                "MSysObjects",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                catalog
                    .columns()
                    .iter()
                    .map(|column| column.name())
                    .collect::<Vec<_>>(),
                [
                    "Id", "ParentId", "Name", "Type", "Flags", "LvProp", "Lv", "LvExtra",
                    "LvModule"
                ]
            );
            for number in [5, 6, 7, 8] {
                assert_eq!(catalog.columns()[number].native_type(), 11);
            }
            let coverage = image
                .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
                .unwrap();
            assert!(std::ptr::eq(coverage.image(), &image));
            assert_eq!(
                coverage.body().presence(),
                if case.body.is_some() {
                    PayloadPresence::Present
                } else {
                    PayloadPresence::Absent
                }
            );
            assert_eq!(
                coverage.module().presence(),
                if case.module.is_some() {
                    PayloadPresence::Present
                } else {
                    PayloadPresence::Absent
                }
            );
            assert_eq!(coverage.body().as_bytes(), case.body.as_deref());
            assert_eq!(coverage.module().as_bytes(), case.module.as_deref());
            assert_eq!(coverage.children().len(), usize::from(case.child));
            if case.child {
                assert_eq!(coverage.children()[0].origin().parent_id(), 10);
            }
            let extra = coverage.table_extra();
            assert_eq!(extra.object_id(), schema.object_id());
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.presence(), PayloadPresence::Present);
            assert_eq!(extra.raw(), Some(case.extra.as_slice()));
            if recipe == model::Recipe::OpaqueExtra {
                assert_eq!(extra.coverage(), TableExtraCoverage::Opaque);
                assert!(extra.groups().is_empty());
            } else {
                assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
                assert_eq!(extra.groups().len(), 1);
                assert_eq!(extra.groups()[0].kind(), 3);
                let properties = extra.groups()[0].properties().unwrap();
                assert_eq!(properties.len(), case.properties.len());
                for (property, (name, xml)) in properties.iter().zip(&case.properties) {
                    assert_eq!(property.name(), name);
                    assert_eq!((property.flags(), property.native_type()), (1, 12));
                    assert_eq!(property.text(), Some(xml.as_str()));
                    let inspected = property
                        .inspect_complete_before_events(&mut budget(), &|| false)
                        .unwrap();
                    assert!(!inspected.envelopes().definitions().is_empty());
                }
                assert_eq!(case.selection, [0]);
            }
            let properties = inspect_table_properties(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let original_properties = inspect_table_properties(
                &mut Cursor::new(&case.original),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(properties.raw_bytes(), original_properties.raw_bytes());
            let names = schema
                .columns()
                .iter()
                .filter(|column| column.scalar_type().is_some())
                .map(|column| column.name())
                .collect::<Vec<_>>();
            let rows = read_table_projection(
                &mut image.reader(),
                SCOPE,
                "Items",
                &names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let original_rows = read_table_projection(
                &mut Cursor::new(&case.original),
                SCOPE,
                "Items",
                &names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), original_rows.rows());
            let id = rows
                .columns()
                .iter()
                .position(|column| column.name() == "ID")
                .unwrap();
            for (ordinal, row) in rows.rows().iter().enumerate() {
                assert_eq!(row[id], Value::Int32(ordinal as i32 + 1));
            }
            let indexes =
                inspect_table_indexes(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(indexes.indexes().len(), 3);
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
            let cached = read_stored_int32_calculation(
                &mut image.reader(),
                SCOPE,
                "Items",
                "Calculated",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(cached.values(), &[Some(2), Some(4), Some(6)]);
            let attachments = read_attachment_column(
                &mut image.reader(),
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
                    .map(|parent| parent.child_count())
                    .collect::<Vec<_>>(),
                [2, 1, 0]
            );
            assert_eq!(image.as_bytes(), case.before);
            sources += 1;
        }
    }
    assert_eq!(sources, 36);
}

#[path = "rich_before_change_sources/required.rs"]
pub(crate) mod required;

#[path = "rich_before_change_sources/expectations.rs"]
pub(crate) mod expectations;
