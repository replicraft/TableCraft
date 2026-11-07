//! System-only construction uses the same required fresh-publication authority.
use super::budget;
use crate::browsing::NativeBrowser;
use crate::creation::{CreationDestination, CreationError};
use std::fs;
use tablecraft_ace::{CreateDatabaseError, InspectError};
use tablecraft_io::publication::PublicationError;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[test]
fn blank_publication_reopens_as_only_system_tables_and_never_clobbers_an_existing_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("blank.accdb");
    let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
        .unwrap()
        .prepare_blank(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        (
            prepared.table_count(),
            prepared.column_count(),
            prepared.row_count()
        ),
        (0, 0, 0),
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let receipt = prepared.publish(&mut budget(), &|| false).unwrap();
    assert!(receipt.namespace_synced());
    assert_eq!(
        (
            receipt.table_count(),
            receipt.column_count(),
            receipt.row_count()
        ),
        (0, 0, 0),
    );
    let expected = canonical_blank_creation::source();
    assert_eq!(fs::read(&path).unwrap(), expected);
    let mut browser = NativeBrowser::new(
        fs::File::open(&path).unwrap(),
        CatalogScope::new(1),
        budget(),
        &|| false,
    );
    let inventory = browser.inventory().unwrap();
    assert_eq!(inventory.tables().count(), 3);
    assert!(inventory.tables().all(|table| table.is_system()));
    assert_eq!(inventory.queries().count(), 0);
    drop(browser);

    let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
        .unwrap()
        .prepare_blank(&mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        prepared.publish(&mut budget(), &|| false),
        Err(CreationError::Publication(
            PublicationError::NotPublished { .. }
        )),
    ));
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn cancelled_blank_preparation_or_publication_creates_no_final_or_staging_entry() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("cancelled.accdb");
    let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        destination.prepare_blank(&mut budget(), &|| true),
        Err(CreationError::Create(CreateDatabaseError::Inspect(
            InspectError::Cancelled
        ))),
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);

    let prepared = CreationDestination::capture(&path, &mut budget(), &|| false)
        .unwrap()
        .prepare_blank(&mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        prepared.publish(&mut budget(), &|| true),
        Err(CreationError::Publication(
            PublicationError::NotPublished { .. }
        )),
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
