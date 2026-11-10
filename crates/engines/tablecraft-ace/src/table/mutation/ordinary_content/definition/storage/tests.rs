//! Independent wide images exercise logical parsing and physical role closure.
use super::*;
use crate::{
    inspect_table_schema,
    table::mutation::ordinary_content::{definition, ledger},
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

const SCOPE: CatalogScope = CatalogScope::new(2550);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn complete_wide_declaration_and_index_names_use_checked_logical_storage() {
    let source = raw::source();
    let image = capture(&source.after);
    let stored = read(&image, 7, &mut budget(), &|| false).unwrap();
    assert_eq!(stored.pages().collect::<Vec<_>>(), [7, 8, 9]);
    assert_eq!(stored.len(), 10_916);
    stored
        .validate_frames(&image, &mut budget(), &|| false)
        .unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Table1", &mut budget(), &|| {
        false
    })
    .unwrap();
    definition::validate_user(&image, &schema, &mut budget(), &|| false).unwrap();
    let mut ledger = ledger::Ledger::new(&image, &mut budget(), &|| false).unwrap();
    let table = definition::capture(
        &image,
        &mut ledger,
        7,
        definition::Role::User,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.layout.fields.len(), 255);
    assert_eq!(table.layout.physical_indexes, 1);
}

#[test]
fn physical_continuation_headers_padding_and_shared_roles_remain_protected() {
    for (offset, value) in [(9 * 4096 + 2, 0), (9 * 4096 + 4095, 0x5a)] {
        let mut source = raw::source();
        source.after[offset] = value;
        let image = capture(&source.after);
        // These ignored bytes do not prevent metadata reading; a source proof
        // still checks the complete original physical frame before any mutation.
        let schema =
            inspect_table_schema(&mut image.reader(), SCOPE, "Table1", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(schema.columns().len(), 255);
        assert!(definition::validate_user(&image, &schema, &mut budget(), &|| false).is_err());
    }
    let source = raw::source();
    let image = capture(&source.after);
    let mut ledger = ledger::Ledger::new(&image, &mut budget(), &|| false).unwrap();
    ledger
        .claim(8, ledger::Role::Data, &mut budget(), &|| false)
        .unwrap();
    assert!(
        definition::capture(
            &image,
            &mut ledger,
            7,
            definition::Role::User,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn wide_storage_uses_caller_budgets_and_cancellation_without_partial_admission() {
    let source = raw::source();
    let image = capture(&source.after);
    assert!(matches!(
        read(
            &image,
            7,
            &mut ResourceBudget::new(0, 100_000, 10_000_000),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        read(&image, 7, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let mut malformed = raw::source();
    malformed.after[8 * 4096 + 4..8 * 4096 + 8].copy_from_slice(&8_u32.to_le_bytes());
    assert!(read(&capture(&malformed.after), 7, &mut budget(), &|| false).is_err());
}
