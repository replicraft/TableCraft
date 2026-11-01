#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_ordinary_fitted::{self as sources, Recipe, Refusal};

const SCOPE: CatalogScope = CatalogScope::new(130);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn locked(path: &std::path::Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> Option<&'static str> {
    match recipe {
        Recipe::NullTextToEmpty => Some("UPDATE Items SET TextValue='' WHERE Id=2"),
        Recipe::EmptyTextToNull => Some("UPDATE Items SET TextValue=NULL WHERE Id=3"),
        Recipe::EmptyBinaryToNull => Some("UPDATE Items SET BinaryValue=NULL WHERE Id=3"),
        Recipe::CompressedText => Some("UPDATE Items SET TextValue='beta' WHERE Id=1"),
        Recipe::RawUnicode => Some("UPDATE Items SET TextValue='Ω雪🙂a' WHERE Id=1"),
        Recipe::EscapedBom => Some("UPDATE Items SET TextValue='\u{feff}' WHERE Id=1"),
        Recipe::ExactNoops => {
            Some("UPDATE Items SET TextValue=TextValue,BinaryValue=BinaryValue WHERE Id=1")
        }
        Recipe::NullNoops => Some("UPDATE Items SET TextValue=NULL,BinaryValue=NULL WHERE Id=2"),
        Recipe::EmptyGroups => {
            Some("UPDATE Items SET TextValue=TextValue,BinaryValue=BinaryValue WHERE Id=-1")
        }
        _ => None,
    }
}

#[test]
fn fitted_sessions_save_all_thirty_typed_and_eighteen_representable_sql_images() {
    let mut typed_saves = 0;
    let mut sql_saves = 0;
    for header in [5, 6] {
        for recipe in sources::RECIPES {
            let case = sources::source(header, recipe);
            let Some(expected) = case.expected.as_ref() else {
                continue;
            };
            for typed in [true, false] {
                let statement = sql(recipe);
                if !typed && statement.is_none() {
                    continue;
                }
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let mut matched = Vec::new();
                for group in &case.groups {
                    for update in &group.updates {
                        matched.push(update.row);
                    }
                }
                matched.sort_unstable();
                matched.dedup();
                let staged = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), case.rows);
                    let cells = case
                        .groups
                        .iter()
                        .map(|group| {
                            group
                                .updates
                                .iter()
                                .map(|update| {
                                    assert_eq!(
                                        &editor.snapshot().rows()[update.row][group.number],
                                        &update.expected
                                    );
                                    CellUpdate::new(
                                        editor.snapshot().row_handle(update.row).unwrap(),
                                        update.replacement.clone(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let columns = case
                        .groups
                        .iter()
                        .zip(&cells)
                        .map(|(group, updates)| ColumnUpdates {
                            column: group.column,
                            updates,
                        })
                        .collect::<Vec<_>>();
                    editor
                        .plan_columns(&columns, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(statement.unwrap(), ParseLimits::default()).unwrap();
                    session
                        .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap_or_else(|error| panic!("h{header} {} SQL: {error:?}", recipe.id()))
                };
                assert_eq!(staged.matched_rows(), matched.len());
                assert_eq!(staged.changed_rows(), case.changed);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(case.changed));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.final_rows);
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                if typed {
                    typed_saves += 1;
                } else {
                    sql_saves += 1;
                }
            }
        }
    }
    assert_eq!((typed_saves, sql_saves), (30, 18));
}

#[test]
fn fitted_capacity_refusals_and_stale_handles_retain_original_files_and_completed_receipt() {
    for header in [5, 6] {
        for recipe in [Recipe::TextOverflow, Recipe::SharedOverflow] {
            let case = sources::source(header, recipe);
            assert_eq!(case.refusal, Some(Refusal::Capacity));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            let cells = case
                .groups
                .iter()
                .map(|group| {
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            CellUpdate::new(
                                editor.snapshot().row_handle(update.row).unwrap(),
                                update.replacement.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let columns = case
                .groups
                .iter()
                .zip(&cells)
                .map(|(group, updates)| ColumnUpdates {
                    column: group.column,
                    updates,
                })
                .collect::<Vec<_>>();
            let failure = editor
                .plan_columns(&columns, &mut budget(), &|| false)
                .err()
                .expect("the physical original capacity is insufficient");
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                    EditError::Unsupported(WriteBlocker::RowLayout)
                )))
            ));
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&path);
            failure.into_session().release().unwrap();
        }
        let case = sources::source(header, Recipe::CompressedText);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "TextValue",
                &[CellUpdate::new(
                    stale,
                    case.groups[0].updates[0].replacement.clone(),
                )],
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
                "TextValue",
                &[CellUpdate::new(
                    stale,
                    case.groups[0].updates[0].replacement.clone(),
                )],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("old snapshot handles cannot become current intent");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(
            fs::read(&path).unwrap().as_slice(),
            case.expected.as_ref().unwrap().as_slice()
        );
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}
