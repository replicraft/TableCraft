use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_relationships};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ReferentialAction;

fn copy() -> Cursor<Vec<u8>> {
    Cursor::new(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../docs/examples/Access_Example_VBA.accdb"),
        )
        .unwrap(),
    )
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn real_relationship_declares_referenced_and_dependent_fields_without_mutation() {
    let mut source = copy();
    let original = source.get_ref().clone();
    let relationships =
        inspect_relationships(&mut source, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    assert_eq!(relationships.len(), 1);
    let relationship = relationships[0].declaration();
    assert_eq!(relationship.parent_table().as_str(), "TAG_GRP_TB");
    assert_eq!(relationship.child_table().as_str(), "TAG_NME_TB");
    assert_eq!(relationship.columns()[0].parent().as_str(), "TAG_GRP_ID");
    assert_eq!(relationship.columns()[0].child().as_str(), "TAG_GRP_ID");
    let rules = relationship.rules().unwrap();
    assert!(rules.enforced);
    assert_eq!(rules.on_delete, ReferentialAction::Restrict);
    assert_eq!(source.get_ref(), &original);
}

#[test]
fn relationship_inspection_shares_catalog_budgets_and_cancellation() {
    assert!(matches!(
        inspect_relationships(&mut copy(), CatalogScope::new(1), &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    for budget in [
        ResourceBudget::new(0, 1000, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 1000, 0),
    ] {
        assert!(matches!(
            inspect_relationships(&mut copy(), CatalogScope::new(1), &mut { budget }, &|| {
                false
            }),
            Err(InspectError::Limit(_))
        ));
    }
}
