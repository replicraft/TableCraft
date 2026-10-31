#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{error::Error, fs, path::Path};
use tablecraft_access::editing::offline::{
    Failure, OfflineWriteSession, Phase, SessionState, StagedUpdate,
};
use tablecraft_ace::IndexConstraintViolation;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_indexed_before_change::{Case, RECIPES, Recipe, source};

#[derive(Clone, Copy, Debug)]
enum Transport {
    Typed,
    Sql,
}

const TRANSPORTS: [Transport; 2] = [Transport::Typed, Transport::Sql];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        // Original Id1/2 independently produce literal user seeds9/-11. The
        // macro's sequential Current/Old statements then yield final20/10.
        Recipe::UniqueSwap => "UPDATE Items SET Score=29-20*Id WHERE Id<=2",
        Recipe::UniqueConflict => "UPDATE Items SET Score=Score-1 WHERE Id=1",
        Recipe::Empty => "UPDATE Items SET Score=Score+7 WHERE Id=-1",
        _ => "UPDATE Items SET Score=Score+7 WHERE Id=1",
    }
}

fn stage(
    owner: OfflineWriteSession,
    case: &Case,
    recipe: Recipe,
    transport: Transport,
    work: &mut ResourceBudget,
) -> Result<StagedUpdate, Failure> {
    match transport {
        Transport::Typed => {
            let editor = owner.edit_table("Items", work, &|| false)?;
            assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
            let updates = case
                .updates
                .iter()
                .map(|update| {
                    let handle = editor.snapshot().row_handle(update.ordinal).unwrap();
                    let row = editor.snapshot().resolve(handle).unwrap();
                    assert_eq!(row[1], Value::Int32(update.expected));
                    CellUpdate::new(handle, Value::Int32(update.replacement))
                })
                .collect::<Vec<_>>();
            // Empty is the actual empty typed request, not synthesized noops.
            assert_eq!(updates.len(), case.updates.len());
            editor.plan_updates("Score", &updates, work, &|| false)
        }
        Transport::Sql => {
            let query = parse_query(sql(recipe), ParseLimits::default()).unwrap();
            owner.plan_update(query.query(), work, None, &|| false)
        }
    }
}

#[test]
fn typed_and_sql_indexed_sessions_match_all_sixteen_complete_literal_images() {
    for header in [5, 6] {
        for recipe in RECIPES {
            let case = source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            for transport in TRANSPORTS {
                let root = tempfile::tempdir().unwrap();
                let database = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&database, &case.before).unwrap();
                let mut work = budget();
                let owner =
                    OfflineWriteSession::open(&database, CatalogScope::new(97), &mut work, &|| {
                        false
                    })
                    .unwrap();
                let staged = stage(owner, &case, recipe, transport, &mut work).unwrap();
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

                let (owner, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), Some(case.updates.len()));
                assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
                assert_eq!(owner.state(), SessionState::Ready);
                assert_eq!(owner.last_completed(), Some(receipt));
                assert_eq!(owner.current_outcome(), None);
                assert_eq!(fs::read(&database).unwrap().as_slice(), expected.as_slice());
                assert!(!bundle.exists());
                locked(&database);

                let editor = owner.edit_table("Items", &mut work, &|| false).unwrap();
                for (ordinal, row) in case.final_rows.iter().enumerate() {
                    let handle = editor.snapshot().row_handle(ordinal).unwrap();
                    assert_eq!(
                        editor.snapshot().resolve(handle).unwrap(),
                        &[Value::Int32(row[0]), Value::Int32(row[1]),]
                    );
                }
                let owner = editor.into_session();
                assert_eq!(owner.last_completed(), Some(receipt));
                locked(&database);
                owner.release().unwrap();
                ExclusiveFile::open(&database).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            }
        }
    }
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
fn both_conflicting_sources_refuse_typed_and_sql_before_any_journal_bundle() {
    for header in [5, 6] {
        let case = source(header, Recipe::UniqueConflict);
        assert!(case.expected.is_none());
        for transport in TRANSPORTS {
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            fs::write(&database, &case.before).unwrap();
            let mut work = budget();
            let owner =
                OfflineWriteSession::open(&database, CatalogScope::new(97), &mut work, &|| false)
                    .unwrap();
            let failure = stage(owner, &case, Recipe::UniqueConflict, transport, &mut work)
                .err()
                .expect("final duplicate key returns no staged update");
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
            ExclusiveFile::open(&database).unwrap().close().unwrap();
        }
    }
}
