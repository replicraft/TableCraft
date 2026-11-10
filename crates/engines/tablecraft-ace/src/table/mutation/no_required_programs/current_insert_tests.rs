//! Current INSERT requires its own source purpose and preserves original rows.
use super::*;
use crate::table::mutation::{normal_ordinary, preserved_source};
use std::io::Cursor;
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, readdition::Target,
    },
    saved_field_rename::fitting::current_history as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(2611);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn current_insert_source_retains_historical_values_and_rejects_other_purpose_certificates() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let before = raw::source(case, target, filled);
                let before = raw::rename(&before, target.name(), "CurrentField");
                let original = image(&before);
                let schema = crate::inspect_table_schema(
                    &mut original.reader(),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let certificate = NoRequiredTablePrograms::capture_current_insert(
                    &original,
                    &schema,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let captured = preserved_source::capture_current_insert(
                    &original,
                    "Table1",
                    &certificate,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(captured.table.rows(), raw::expected(case, target, filled));
                assert_eq!(captured.records.len(), captured.table.rows().len());
                certificate
                    .require_current_insert_source(&original, &schema, &mut budget(), &|| false)
                    .unwrap();
                assert!(matches!(
                    certificate.require_source(&original, &schema, &mut budget(), &|| false),
                    Err(EditError::Preservation)
                ));
                assert!(matches!(
                    certificate.require_current_schema_source(
                        &original,
                        &schema,
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                ));
                assert!(matches!(
                    certificate.require_current_rows_source(
                        &original,
                        &schema,
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                ));
                assert!(matches!(
                    certificate.require_generated_insert(),
                    Err(EditError::Preservation)
                ));

                let readonly = NoRequiredTablePrograms::capture_current_schema(
                    &original,
                    &schema,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(matches!(
                    preserved_source::capture_current_insert(
                        &original,
                        "Table1",
                        &readonly,
                        &mut budget(),
                        &|| false,
                    ),
                    Err(EditError::Preservation)
                ));
                let updates = NoRequiredTablePrograms::capture_current_rows(
                    &original,
                    &schema,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(matches!(
                    preserved_source::capture_current_insert(
                        &original,
                        "Table1",
                        &updates,
                        &mut budget(),
                        &|| false,
                    ),
                    Err(EditError::Preservation)
                ));
                let fresh = image(&before);
                assert!(matches!(
                    certificate.require_current_insert_source(
                        &fresh,
                        &schema,
                        &mut budget(),
                        &|| false,
                    ),
                    Err(EditError::Preservation)
                ));
                assert_eq!(original.as_bytes(), before);
            }
        }
    }
}

#[test]
fn current_insert_source_honors_cancellation_and_each_cumulative_resource_pool() {
    let before = raw::source(CASES[0], Target::ShortText, true);
    let original = image(&before);
    assert!(matches!(
        normal_ordinary::capture_current_insert(&original, "Table1", &mut budget(), &|| true,),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut pool = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(512 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(512 * 1024 * 1024, 100_000, 0),
        };
        assert!(matches!(normal_ordinary::capture_current_insert(
            &original, "Table1", &mut pool, &|| false,
        ), Err(EditError::Inspect(crate::InspectError::Limit(error))) if error.resource == resource));
    }
}
