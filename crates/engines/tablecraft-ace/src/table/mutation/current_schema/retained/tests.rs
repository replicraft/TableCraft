//! Retained metadata proofs protect the complete cohort without row authority.
use super::*;
use crate::{
    EditError,
    table::mutation::{current_schema, no_required_programs::NoRequiredTablePrograms},
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields::{
    self as raw, CASES, Target,
};

const SCOPE: CatalogScope = CatalogScope::new(4001);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_schema_observes_inline_and_external_properties_after_historical_additions() {
    for case in CASES {
        let raw = raw::source(case);
        let original = image(&raw.bytes);
        let before = observe(&original, "Table1", &mut budget(), &|| false).unwrap();
        assert!(before.family == Family::Retained);
        assert_eq!(before.schema.definition_page(), raw.definition);
        assert_eq!(before.schema.declared_rows() as usize, raw.roots.len());
        assert!(current_schema::observe(&original, "Table1", &mut budget(), &|| false).is_err());

        for target in [Target::Number, Target::ShortText] {
            let after = raw::after(&raw.bytes, raw.definition, raw.catalog_root, target);
            let updated = image(&after);
            let current = observe(&updated, "Table1", &mut budget(), &|| false).unwrap();
            assert_eq!(current.schema.object_id(), before.schema.object_id());
            assert_eq!(
                current.schema.columns().len(),
                before.schema.columns().len() + 1
            );
            assert_eq!(current.database.raw_bytes(), before.database.raw_bytes());
            assert!(matches!(current.user_storage, UserStorage::Single { .. }));
        }
    }
}

#[test]
fn retained_schema_certificates_cannot_be_substituted_for_row_program_purposes() {
    for case in CASES {
        let source = raw::source(case);
        let original = image(&source.bytes);
        let table = inspect_table_schema(
            &mut original.reader(),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let schema = NoRequiredTablePrograms::capture_retained_schema(
            &original,
            &table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        schema
            .require_retained_schema_source(&original, &table, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            schema.require_retained_rows_source(&original, &table, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            schema.require_retained_delete_source(&original, &table, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            schema.require_retained_insert_source(&original, &table, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        let rows = NoRequiredTablePrograms::capture_retained_rows(
            &original,
            &table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            rows.require_retained_schema_source(&original, &table, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
    }
}
