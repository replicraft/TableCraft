#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, EditableTable, SnapshotError};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_binary_delete::{self as sources, Recipe};

const SCOPE: CatalogScope = CatalogScope::new(220);
const PAGE_BYTES: usize = 4096;
const FREE_BYTES_OFFSET: usize = 2;

fn sequential_after(first: &sources::Case, both: &sources::Case) -> Vec<u8> {
    let mut expected = both.after.clone();

    // The frozen atomic result supplies final active membership. Two commits
    // retain the inactive tail from the intermediate source, rather than the
    // tail from the original source used by the atomic two-row deletion.
    for root in both.roots {
        let base = root as usize * PAGE_BYTES;
        let free_at = base + FREE_BYTES_OFFSET;
        let free = u16::from_le_bytes(
            both.after[free_at..free_at + size_of::<u16>()]
                .try_into()
                .unwrap(),
        );
        let tail = base + PAGE_BYTES - usize::from(free);
        let end = base + PAGE_BYTES;
        expected[tail..end].copy_from_slice(&first.after[tail..end]);
    }

    let hash = match both.header {
        5 => "c4e8f05e66ead90d20d3eafd43fbc9233fd1813437430a264ad06c1bc88f9c4f",
        6 => "a3ce65f245e542629eb9668f9d742af5d22409ebe4a2a27176ea7eca810226e0",
        _ => unreachable!(),
    };
    verify(&expected, hash);
    assert_ne!(expected, both.after);
    expected
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires14 independently native/SDK-qualified Binary DELETE images"]
fn duplicate_deletions_reload_shifted_visible_handle_on_the_same_lease() {
    // Every original and AFTER pin is checked before either first commit.
    let cases = sources::load_all(verify);
    let mut commits = 0;
    for header in [5_u8, 6] {
        let first = cases
            .iter()
            .find(|case| case.header == header && case.recipe == Recipe::OneDuplicate)
            .unwrap();
        let both = cases
            .iter()
            .find(|case| case.header == header && case.recipe == Recipe::BothDuplicates)
            .unwrap();
        assert_eq!(first.before, both.before);
        let expected = sequential_after(first, both);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &first.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(snapshot.rows(), sources::original_rows());
        let removed_handle = snapshot.row_handle(2).unwrap();
        let remaining_old_handle = snapshot.row_handle(12).unwrap();
        let staged =
            DeletableTable::plan_deletes(editor, &[removed_handle], &mut budget(), &|| false)
                .unwrap();
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
        assert_eq!(fs::read(&path).unwrap(), first.before);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), Some(1));
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), first.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        commits += 1;

        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(snapshot.rows(), sources::final_rows(Recipe::OneDuplicate));
        assert_eq!(snapshot.rows()[11][0], Value::Int32(13));
        assert_eq!(snapshot.rows()[11][1], Value::Binary(vec![0]));
        assert_eq!(
            snapshot.resolve(removed_handle),
            Err(SnapshotError::StaleRow)
        );
        assert_eq!(
            snapshot.resolve(remaining_old_handle),
            Err(SnapshotError::StaleRow)
        );
        let failure =
            DeletableTable::plan_deletes(editor, &[remaining_old_handle], &mut budget(), &|| false)
                .err()
                .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), first.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

        let editor = failure
            .into_session()
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        // Original physical slot12 is now visible ordinal11, not ordinal12.
        assert_eq!(snapshot.rows().len(), 12);
        assert_eq!(snapshot.rows()[11][0], Value::Int32(13));
        assert!(snapshot.row_handle(12).is_none());
        let fresh = snapshot.row_handle(11).unwrap();
        let staged =
            DeletableTable::plan_deletes(editor, &[fresh], &mut budget(), &|| false).unwrap();
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (1, 1));
        let (session, second) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(second.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(second.matched_rows(), Some(1));
        assert_eq!(second.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(second));
        assert!(fs::read(&path).unwrap() == expected);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        commits += 1;

        let editor = session
            .edit_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            EditableTable::snapshot(&editor).rows(),
            sources::final_rows(Recipe::BothDuplicates)
        );
        editor.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert!(fs::read(&path).unwrap() == expected);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert_eq!(commits, 4);
}
