#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};
use tablecraft_access::editing::{
    NativeEditError,
    journal::{self, NativeJournalInserter},
    offline::OfflineWriteSession,
};
use tablecraft_ace::{InspectError, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::InsertableTable;
use tablecraft_io::{ExclusiveFile, IoError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_ordinary_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(162);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn metadata_constructor_does_not_scan_corrupt_target_records_but_finish_refuses() {
    for case in sources::load_all(verify) {
        // Negative authored derivative only. Both immutable originals and both
        // source-qualified AFTERs were verified by the shared loader first.
        // Change the old row's variable count from literal 2 to 65535 without
        // changing its TD, catalog, field names, raw payload or record boundary.
        let mut negative = case.before.clone();
        let count_at = case.data as usize * 4096 + 4093;
        assert_eq!(&negative[count_at..count_at + 3], &[2, 0, 7]);
        negative[count_at..count_at + 2].copy_from_slice(&[255, 255]);
        assert_ne!(negative, case.before);
        assert_eq!(negative.len(), case.before.len());

        let inserter =
            NativeJournalInserter::new(capture(&negative), "Sentinel", &mut budget(), &|| false)
                .unwrap();
        assert_eq!(inserter.name(), "Sentinel");
        assert_eq!(
            inserter
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            ["ID", "Marker", "Bytes"],
        );
        let error = inserter
            .plan_insert(sources::values(), &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(matches!(error, journal::Error::Plan(_)));

        // The real retained consumer also exposes metadata successfully, then
        // refuses during complete native admission before it can create a pair.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("negative.accdb");
        let bundle = directory.path().join("never-created-journal");
        fs::write(&path, &negative).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let inserter = session
            .insert_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let failure = inserter
            .plan_insert(sources::values(), &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(failure.invalid_input());
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.pair_state(), None);
        assert!(!bundle.exists());
        assert_eq!(fs::read(&path).unwrap(), negative);
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn metadata_constructor_retains_cancellation_and_resource_failures() {
    for case in sources::load_all(verify) {
        assert!(matches!(
            NativeJournalInserter::new(capture(&case.before), "Sentinel", &mut budget(), &|| true,),
            Err(NativeEditError::Inspect(InspectError::Cancelled))
        ));
        for mut limit in [
            ResourceBudget::new(0, 1_000_000, 1_000_000_000),
            ResourceBudget::new(1_000_000_000, 1_000_000, 0),
        ] {
            assert!(matches!(
                NativeJournalInserter::new(capture(&case.before), "Sentinel", &mut limit, &|| {
                    false
                },),
                Err(NativeEditError::Inspect(InspectError::Limit(_)))
            ));
        }
    }
}
