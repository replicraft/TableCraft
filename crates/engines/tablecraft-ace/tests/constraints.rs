use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    IndexConstraintViolation as V, InspectError, check_table, inspect_table_indexes,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn copy() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn check(bytes: Vec<u8>) -> Result<tablecraft_ace::TableCheck, InspectError> {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn declared_unique_and_required_rules_are_checked_without_mutating_the_original() {
    let original = copy();
    // Independently observed: table definition 726; physical COLOR_ID flags at
    // byte 276. Its repeated, non-NULL Int32 keys form a valid non-unique index.
    let mut bytes = original.clone();
    bytes[726 * 4096 + 276] = 0x81;
    assert_eq!(
        check(bytes).err(),
        Some(InspectError::IndexConstraint(V::DuplicateKey))
    );
    let mut bytes = original.clone();
    bytes[726 * 4096 + 409] = 1; // Logical COLOR_ID declares a primary key.
    assert_eq!(
        check(bytes).err(),
        Some(InspectError::IndexConstraint(V::DuplicateKey))
    );
    let mut bytes = original.clone();
    bytes[726 * 4096 + 276] = 0x88; // Required non-unique, all original keys present.
    assert!(check(bytes).is_ok());
    assert_eq!(copy(), original);
}

#[test]
fn primary_backing_is_shared_and_multiple_primary_declarations_are_rejected() {
    let mut bytes = copy();
    bytes[726 * 4096 + 409] = 1;
    // Logical index 2 shares physical index 0. Its own logical type stays ordinary.
    bytes[726 * 4096 + 450..726 * 4096 + 454].copy_from_slice(&0_u32.to_le_bytes());
    let inspection = inspect_table_indexes(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let alias = inspection.indexes()[2].constraints().unwrap();
    assert!(!alias.primary_key() && alias.unique() && alias.required());
    bytes[726 * 4096 + 465] = 1;
    assert_eq!(
        inspect_table_indexes(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
}
