//! Represented catalog candidates grant observation, not schema/LVAL mutation.
use super::{Declaration, NoRequiredTablePrograms, Purpose};
use crate::{
    CreateColumn, EditError, FirstTableDefinition, LongValueMapReferences, NativeImage,
    WriteBlocker, inspect_table_schema,
    table::{
        mutation::owned_lval::maps::{EofAllocation, Maps},
        table_append::Inventory,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::retained_catalog_gap;

const SCOPE: CatalogScope = CatalogScope::new(7392);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

#[test]
fn represented_catalog_candidates_admit_only_retained_data_certificates() {
    // The independently authored source has a real MSysDb and a small catalog
    // gap. Only the requested Description forces external property placement.
    let before = retained_catalog_gap::source();
    let zero = Value::Int32(0);
    let description = "D".repeat(255);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
            .with_description(&description)
            .unwrap(),
    ];
    let declaration =
        FirstTableDefinition::capture_authored("Described", &columns, &mut budget(), &|| false)
            .unwrap();
    let image =
        NativeImage::capture_retained(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
            .unwrap();
    let plan = image
        .plan_authored_add_table(declaration, &mut budget(), &|| false)
        .unwrap();
    assert!(plan.is_native());
    let image = NativeImage::capture_retained(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let schema = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "Described",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    Inventory::capture_represented(&image, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        Inventory::capture(&image, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));

    for purpose in [
        Purpose::RetainedInsert,
        Purpose::RetainedRows,
        Purpose::RetainedOpaqueRows,
        Purpose::RetainedOpaqueInsert,
        Purpose::RetainedDelete,
    ] {
        let certificate = NoRequiredTablePrograms::capture_with_purpose(
            &image,
            &schema,
            Declaration::CurrentSchema,
            purpose,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            certificate.require_retained_schema_source(&image, &schema, &mut budget(), &|| false,),
            Err(EditError::Preservation)
        ));
        if purpose != Purpose::RetainedInsert {
            assert!(matches!(
                certificate
                    .require_retained_insert_source(&image, &schema, &mut budget(), &|| false,),
                Err(EditError::Preservation)
            ));
        }
    }
    assert!(matches!(
        NoRequiredTablePrograms::capture_retained_schema(&image, &schema, &mut budget(), &|| false,),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));

    let catalog = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    // Actual packed catalog LvProp declaration cells in this independent seed.
    let references = LongValueMapReferences {
        owned: 3 * 256 + 16,
        candidate: 3 * 256 + 17,
    };
    assert!(matches!(
        Maps::capture_with_eof_policy(
            &image,
            &catalog,
            references,
            EofAllocation::RequireAllocated,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}
