//! Child of the existing mixed session integration owner; shared host helpers.
use super::{SCOPE, Transport, budget, locked};
use std::{error::Error, fs, io::Cursor};
use tablecraft_access::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
    offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate},
};
use tablecraft_ace::{IndexConstraintViolation, NativeImage};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_indexed_mixed_before_change::{
    self as model, Case, Recipe,
};

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::CounterUniqueSwap => {
            // Original Id1/3 give user seeds1999/-2003. Native Current+Id+Old
            // then yields Score3000/1000, never two independently staged saves.
            "UPDATE Items SET Score=4000-2001*Id WHERE Id=1 OR Id=3"
        }
        Recipe::CounterUniqueConflict | Recipe::CounterCompositeTie => {
            "UPDATE Items SET Score=999 WHERE Id=1"
        }
        Recipe::CounterLeafPrefixLoss => "UPDATE Items SET Score=2147482646 WHERE Id=1",
        Recipe::CounterTreePrefixLoss => "UPDATE Items SET Score=-3103 WHERE Id=3",
        _ => "UPDATE Items SET Score=2599 WHERE Id=1",
    }
}

fn stage(
    session: OfflineWriteSession,
    case: &Case,
    recipe: Recipe,
    transport: Transport,
    work: &mut ResourceBudget,
) -> Result<StagedUpdate, Failure> {
    match transport {
        Transport::Typed => {
            let editor = session.edit_table("Items", work, &|| false)?;
            // Counter originals have no unindexed readable I32 candidate: Id
            // is generated/primary and Score is indexed. This requires the real
            // checked indexed-target context, not the old first-field shortcut.
            assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
            let updates = case
                .updates
                .iter()
                .map(|update| {
                    let handle = editor.snapshot().row_handle(update.ordinal).unwrap();
                    assert_eq!(
                        editor.snapshot().resolve(handle).unwrap(),
                        case.rows[update.ordinal]
                    );
                    CellUpdate::new(handle, Value::Int32(update.replacement))
                })
                .collect::<Vec<_>>();
            editor.plan_updates("Score", &updates, work, &|| false)
        }
        Transport::Sql => {
            let query = parse_query(sql(recipe), ParseLimits::default()).unwrap();
            session.plan_update(query.query(), work, None, &|| false)
        }
    }
}

#[test]
fn indexed_mixed_typed_and_sql_sessions_publish_twenty_four_literal_complete_images() {
    let mut saved = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            for transport in [Transport::Typed, Transport::Sql] {
                let root = tempfile::tempdir().unwrap();
                let database = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&database, &case.before).unwrap();
                let mut work = budget();
                let session =
                    OfflineWriteSession::open(&database, SCOPE, &mut work, &|| false).unwrap();
                let staged = stage(session, &case, recipe, transport, &mut work).unwrap();
                assert_eq!(
                    staged.matched_rows(),
                    case.updates.len(),
                    "{recipe:?}/{transport:?}"
                );
                assert_eq!(
                    staged.changed_rows(),
                    case.changed_rows,
                    "{recipe:?}/{transport:?}"
                );
                assert_eq!(fs::read(&database).unwrap(), case.before);
                assert!(!bundle.exists());
                locked(&database);
                let (session, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), Some(case.updates.len()));
                assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
                assert_eq!(session.state(), SessionState::Ready);
                assert_eq!(session.last_completed(), Some(receipt));
                let actual = fs::read(&database).unwrap();
                assert_eq!(actual, *expected);
                if case.primary {
                    assert_eq!(
                        &actual[9 * 4096..10 * 4096],
                        &case.before[9 * 4096..10 * 4096]
                    );
                }
                assert!(!bundle.exists());
                locked(&database);
                let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
                for (ordinal, row) in case.final_rows.as_ref().unwrap().iter().enumerate() {
                    let handle = editor.snapshot().row_handle(ordinal).unwrap();
                    assert_eq!(editor.snapshot().resolve(handle).unwrap(), row);
                }
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&database).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saved += 1;
            }
        }
    }
    assert_eq!(saved, 24);
}

fn duplicate_key(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.downcast_ref::<IndexConstraintViolation>()
            == Some(&IndexConstraintViolation::DuplicateKey)
        {
            return true;
        }
        current = error.source();
    }
    false
}

#[test]
fn indexed_mixed_unique_conflicts_refuse_both_hosts_before_any_bundle() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterUniqueConflict);
        assert!(case.expected.is_none());
        for transport in [Transport::Typed, Transport::Sql] {
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            fs::write(&database, &case.before).unwrap();
            let mut work = budget();
            let session =
                OfflineWriteSession::open(&database, SCOPE, &mut work, &|| false).unwrap();
            let failure = stage(
                session,
                &case,
                Recipe::CounterUniqueConflict,
                transport,
                &mut work,
            )
            .err()
            .expect("final unique collision cannot stage a session save");
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert!(duplicate_key(&failure));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&database).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&database);
            failure.into_session().release().unwrap();
        }
    }
}

#[test]
fn indexed_mixed_checked_editor_rejects_stale_and_foreign_handles_on_the_same_lease() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterCompositeTie);
        let expected = case.expected.as_ref().unwrap();
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("target.accdb");
        fs::write(&database, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&database, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(999))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (mut session, receipt) = staged
            .apply(&root.path().join("journal"), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fs::read(&database).unwrap(), *expected);
        let foreign_image = NativeImage::capture(
            &mut Cursor::new(expected),
            CatalogScope::new(306),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let foreign =
            NativeJournalEditor::new(foreign_image, "Items", &mut budget(), &|| false).unwrap();
        assert_eq!(foreign.update_profile(), UpdateProfile::ExistingInt32);
        let foreign_handle = foreign.snapshot().row_handle(0).unwrap();
        for handle in [stale, foreign_handle] {
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                editor.snapshot().resolve(handle),
                Err(SnapshotError::StaleRow)
            );
            let failure = editor
                .plan_updates(
                    "Score",
                    &[CellUpdate::new(handle, Value::Int32(29))],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("stale/foreign snapshot is not a native indexed request");
            assert_eq!(failure.phase(), Phase::Planning);
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                    SnapshotError::StaleRow
                )))
            ));
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&database).unwrap(), *expected);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&database);
            session = failure.into_session();
        }
        session.release().unwrap();
    }
}
