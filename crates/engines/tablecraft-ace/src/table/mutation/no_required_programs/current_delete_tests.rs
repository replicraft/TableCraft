//! Current deletion requires its own source purpose, never another action token.
use super::*;
use crate::table::mutation::{normal_actions, preserved_source};
use std::io::Cursor;
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    current_delete as raw,
    deleted_field::populated::{Case, readdition::Target},
};

const SCOPE: CatalogScope = CatalogScope::new(2711);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn current_delete_source_preserves_rows_and_rejects_other_action_purposes() {
    for (case, target) in [
        (Case::MixedBinary, Target::ShortText),
        (Case::IdOnlyText, Target::Number),
    ] {
        let original = raw::source(case, target, true, true, true);
        let native = image(&original.bytes);
        let schema = crate::inspect_table_schema(
            &mut native.reader(),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let deletion = NoRequiredTablePrograms::capture_current_delete(
            &native,
            &schema,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let captured = preserved_source::capture_current_delete(
            &native,
            "Table1",
            &deletion,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(captured.table.rows(), original.rows);
        assert_eq!(captured.records.len(), original.roots.len());
        assert!(
            deletion
                .require_current_delete_source(&native, &schema, &mut budget(), &|| false)
                .is_ok()
        );
        assert!(matches!(
            deletion.require_source(&native, &schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            deletion.require_current_schema_source(&native, &schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            deletion.require_current_rows_source(&native, &schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            deletion.require_current_insert_source(&native, &schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        let readonly = NoRequiredTablePrograms::capture_current_schema(
            &native,
            &schema,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let insertion = NoRequiredTablePrograms::capture_current_insert(
            &native,
            &schema,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let updates =
            NoRequiredTablePrograms::capture_current_rows(&native, &schema, &mut budget(), &|| {
                false
            })
            .unwrap();
        for other in [&readonly, &insertion, &updates] {
            assert!(matches!(
                preserved_source::capture_current_delete(
                    &native,
                    "Table1",
                    other,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
        }
        let fresh = image(&original.bytes);
        assert!(matches!(
            deletion.require_current_delete_source(&fresh, &schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert_eq!(native.as_bytes(), original.bytes);
    }
}

#[test]
fn current_delete_source_honors_cancellation_and_cumulative_resource_pools() {
    let original = raw::source(Case::MixedBinary, Target::ShortText, true, true, true);
    let native = image(&original.bytes);
    assert!(matches!(
        normal_actions::capture_current_delete(&native, "Table1", &mut budget(), &|| true),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut pool = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(512 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(512 * 1024 * 1024, 100_000, 0),
        };
        assert!(
            matches!(normal_actions::capture_current_delete(&native, "Table1", &mut pool, &|| false), Err(EditError::Inspect(crate::InspectError::Limit(error))) if error.resource == resource)
        );
    }
}
