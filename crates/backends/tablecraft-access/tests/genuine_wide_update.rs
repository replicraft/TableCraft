#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_wide_update::{self as source, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    source::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn position(column: &str) -> usize {
    match column {
        "Id" => 0,
        "KeyValue" => 1,
        "Marker" => 2,
        _ => panic!("outside frozen original schema"),
    }
}

#[test]
#[ignore = "requires qualified wider UPDATE whole-image source views"]
fn retained_typed_and_sql_wider_updates_preserve_images_counts_and_lease() {
    let cases = cases();
    let mut saves = [0_usize; 2];
    for case in cases {
        for typed in [true, false] {
            if !typed && case.recipe != "present-to-null" {
                continue;
            }
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let journal = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(282), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            let stale = editor.snapshot().row_handle(0).unwrap();
            let mut expected_rows = editor.snapshot().rows().to_vec();
            let mut matched = Vec::new();
            for (column, assignments) in &case.groups {
                for assignment in assignments {
                    assert_eq!(
                        expected_rows[assignment.row][position(column)],
                        assignment.expected
                    );
                    expected_rows[assignment.row][position(column)] =
                        source::cell(&assignment.replacement);
                    matched.push(assignment.row);
                }
            }
            matched.sort_unstable();
            matched.dedup();
            let staged = if typed {
                let cells: Vec<_> = case
                    .groups
                    .iter()
                    .map(|(column, assignments)| {
                        let values: Vec<_> = assignments
                            .iter()
                            .map(|assignment| {
                                CellUpdate::new(
                                    editor.snapshot().row_handle(assignment.row).unwrap(),
                                    source::cell(&assignment.replacement),
                                )
                            })
                            .collect();
                        (*column, values)
                    })
                    .collect();
                let columns: Vec<_> = cells
                    .iter()
                    .map(|(column, values)| ColumnUpdates {
                        column,
                        updates: values,
                    })
                    .collect();
                editor
                    .plan_columns(&columns, &mut budget(), &|| false)
                    .unwrap()
            } else {
                let session = editor.into_session();
                let text = format!(
                    "UPDATE [{}] SET [KeyValue] = NULL WHERE [Marker] = 17",
                    case.table
                );
                let query = parse_query(&text, ParseLimits::default()).unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!(
                (staged.matched_rows(), staged.changed_rows()),
                (matched.len(), case.changed)
            );
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(matched.len()));
            assert_eq!(receipt.changed_rows(), Some(case.changed));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!journal.exists());
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), expected_rows);
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            if case.changed > 0 {
                let failure = editor
                    .plan_updates(
                        "Marker",
                        &[CellUpdate::new(stale, source::cell(&expected_rows[0][2]))],
                        &mut budget(),
                        &|| false,
                    )
                    .err()
                    .unwrap();
                assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                        SnapshotError::StaleRow
                    )))
                ));
                assert_eq!(failure.state(), SessionState::Ready);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!journal.exists());
                failure.into_session().release().unwrap();
            } else {
                editor.into_session().release().unwrap();
            }
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [48, 8]);
}
