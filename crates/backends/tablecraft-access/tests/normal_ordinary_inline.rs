#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, io::Cursor, path::Path};
use tablecraft_access::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_ordinary_inline::{
    self as sources, Case, Source,
};

const SCOPE: CatalogScope = CatalogScope::new(121);
const PAGE: usize = 4096;
const POSITIVES: [Case; 5] = [
    Case::WideOne,
    Case::WideExtrema,
    Case::DifferentWindows,
    Case::FutureFree,
    Case::EmptyWindows,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn source(header: u8, case: Case) -> Source {
    let mut source = sources::source(case);
    source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    source
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn expected(source: &Source) -> Vec<u8> {
    assert_eq!(source.data, 95);
    assert_eq!(source.definition, 7);
    let base = 95 * PAGE;
    assert_eq!(&source.bytes[base + 4..base + 8], &7_u32.to_le_bytes());
    let start = usize::from(u16::from_le_bytes(
        source.bytes[base + 14..base + 16].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    assert_eq!(
        &source.bytes[base + start..base + start + 2],
        &2_u16.to_le_bytes()
    );
    let at = base + start + 6;
    assert_eq!(&source.bytes[at..at + 4], &source.rows[0][1].to_le_bytes());
    let mut after = source.bytes.clone();
    after[at..at + 4].copy_from_slice(&17_i32.to_le_bytes());
    after
}

#[test]
fn ten_high_page_plans_keep_original_fixed_profile_four() {
    for header in [5, 6] {
        for case in POSITIVES {
            let source = source(header, case);
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let editor =
                NativeJournalEditor::new(image, "Items", &mut budget(), &|| false).unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            let row = editor.snapshot().row_handle(0).unwrap();
            let plan = editor
                .plan_updates(
                    "Score",
                    &[CellUpdate::new(row, Value::Int32(17))],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert_eq!(
                &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
                b"TCNI0004"
            );
            assert_eq!(plan.after_bytes(), expected(&source));
            assert_eq!(plan.changed_rows(), 1);
        }
    }
}

#[test]
fn typed_and_sql_publish_twenty_complete_high_page_images() {
    let query = parse_query(
        "UPDATE Items SET Score=17 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let mut saves = 0;
    for header in [5, 6] {
        for case in POSITIVES {
            let source = source(header, case);
            let after = expected(&source);
            for typed in [true, false] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    let row = editor.snapshot().row_handle(0).unwrap();
                    editor
                        .plan_updates(
                            "Score",
                            &[CellUpdate::new(row, Value::Int32(17))],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), after);
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                let mut rows = source
                    .rows
                    .iter()
                    .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                rows[0][1] = Value::Int32(17);
                assert_eq!(editor.snapshot().rows(), rows);
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 20);
}

#[test]
fn high_page_noops_and_empty_requests_retire_without_changing_bytes() {
    for header in [5, 6] {
        let source = source(header, Case::FutureFree);
        for typed in [true, false] {
            for empty in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    let updates = if empty {
                        Vec::new()
                    } else {
                        vec![CellUpdate::new(
                            editor.snapshot().row_handle(0).unwrap(),
                            Value::Int32(10),
                        )]
                    };
                    editor
                        .plan_updates("Score", &updates, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let sql = if empty {
                        "UPDATE Items SET Score=10 WHERE Id=-1"
                    } else {
                        "UPDATE Items SET Score=10 WHERE Id=1"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (staged.matched_rows(), staged.changed_rows()),
                    (usize::from(!empty), 0)
                );
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.changed_rows(), Some(0));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                session.release().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            }
        }
    }
}

#[test]
fn invalid_map_closure_and_stale_handles_preserve_resources_and_receipts() {
    for header in [5, 6] {
        for case in [
            Case::MissingGlobalCoverage,
            Case::OwnedPastEof,
            Case::CandidatePastEof,
            Case::MapAlias,
        ] {
            let source = source(header, case);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            fs::write(&path, &source.bytes).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            // A malformed owned map may fail scalar capture itself. Otherwise
            // the complete closure must leave this editor unavailable.
            let failure = match session.edit_table("Items", &mut budget(), &|| false) {
                Err(failure) => failure,
                Ok(editor) => {
                    assert_eq!(editor.update_profile(), UpdateProfile::Unavailable);
                    let row = editor.snapshot().row_handle(0).unwrap();
                    editor
                        .plan_updates(
                            "Score",
                            &[CellUpdate::new(row, Value::Int32(17))],
                            &mut budget(),
                            &|| false,
                        )
                        .err()
                        .expect("invalid maps cannot produce a staged update")
                }
            };
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&path);
            failure.into_session().release().unwrap();
        }

        let source = source(header, Case::DifferentWindows);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(19))],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("a refreshed snapshot rejects the prior handle");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read(&path).unwrap(), expected(&source));
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}
