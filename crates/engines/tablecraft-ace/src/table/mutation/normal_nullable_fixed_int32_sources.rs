//! Read-only qualification before normal nullable Int32 user/macro writes.
use super::{guards::native_events, preserved_source};
use crate::{NativeImage, RuleCheckStatus, check_table, check_table_constraints, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_nullable_fixed_int32_before_change::{
    self as model, Recipe,
};

const SCOPE: CatalogScope = CatalogScope::new(112);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn nullable_int32_originals_and_literal_null_macro_results_qualify_before_normal_writes() {
    let mut views = 0;
    for header in [5, 6] {
        for recipe in [Recipe::Activate, Recipe::Clear] {
            let case = model::source(header, recipe);
            for (bytes, rows) in [
                (&case.before, &case.rows),
                (&case.expected, &case.final_rows),
            ] {
                let image =
                    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                        .unwrap();
                let checked =
                    check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(checked.rows(), 3);
                assert!(checked.indexes().is_empty());
                for (number, name) in [(0, "Id"), (5, "Score")] {
                    let field = &checked.schema().columns()[number];
                    assert_eq!(field.name(), name);
                    assert_eq!(
                        (field.number(), field.native_type(), field.flags()),
                        (number, 4, 3)
                    );
                    assert_eq!((field.storage_length(), field.extended_flags()), (4, 0));
                }
                let actual =
                    read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(actual.rows(), rows);
                let constraints = check_table_constraints(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(constraints.rows(), 3);
                assert!(constraints.relationships().is_empty());
                assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
                let selected =
                    native_events::programs(&image, checked.schema(), &mut budget(), &|| false)
                        .unwrap();
                let source = preserved_source::capture_native_before_change(
                    &image,
                    "Items",
                    &selected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(source.table.rows(), rows);
                assert!(source.indexes.indexes().is_empty());
                let coverage = image
                    .inspect_table_macro_coverage(
                        checked.schema().object_id(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(coverage.table_extra().raw(), Some(case.extra.as_slice()));
                assert_eq!(image.as_bytes(), bytes);
                views += 1;
            }
        }
    }
    assert_eq!(views, 8);
}

mod export;
