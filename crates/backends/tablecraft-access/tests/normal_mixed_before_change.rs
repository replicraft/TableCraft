#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, path::Path};
use tablecraft_access::editing::offline::{
    Failure, OfflineWriteSession, SessionState, StagedUpdate,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_mixed_before_change::{
    self as model, Case, Recipe,
};

const SCOPE: CatalogScope = CatalogScope::new(102);

#[derive(Clone, Copy, Debug)]
enum Transport {
    Typed,
    Sql,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::FixedSequential | Recipe::FixedRestore => {
            unreachable!("fixed signed-zero requests use typed ordinal handles")
        }
        Recipe::CounterNullEmpty => "UPDATE Items SET Score=17 WHERE Id=2",
        Recipe::CounterEmpty => "UPDATE Items SET Score=17 WHERE Id=0",
        _ => "UPDATE Items SET Score=17 WHERE Id=1",
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
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
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
            editor.plan_updates(case.column, &updates, work, &|| false)
        }
        Transport::Sql => {
            let query = parse_query(sql(recipe), ParseLimits::default()).unwrap();
            owner.plan_update(query.query(), work, None, &|| false)
        }
    }
}

#[test]
fn mixed_scalar_typed_and_sql_sessions_preserve_siblings_and_counter_indexes() {
    let mut completed = 0;

    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };

            for transport in [Transport::Typed, Transport::Sql] {
                if matches!(transport, Transport::Sql)
                    && matches!(recipe, Recipe::FixedSequential | Recipe::FixedRestore)
                {
                    // Both fixed rows compare alike under supported SQL
                    // predicates; ordinal handles select the frozen request.
                    continue;
                }

                let root = tempfile::tempdir().unwrap();
                let database = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&database, &case.before).unwrap();
                let mut work = budget();
                let owner =
                    OfflineWriteSession::open(&database, SCOPE, &mut work, &|| false).unwrap();
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
                assert_eq!(fs::read(&database).unwrap(), *expected);
                assert!(!bundle.exists());
                locked(&database);

                let editor = owner.edit_table("Items", &mut work, &|| false).unwrap();
                for (ordinal, row) in case.final_rows.as_ref().unwrap().iter().enumerate() {
                    let handle = editor.snapshot().row_handle(ordinal).unwrap();
                    assert_eq!(editor.snapshot().resolve(handle).unwrap(), row);
                }
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&database).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                completed += 1;
            }
        }
    }

    assert_eq!(completed, 28);
}

#[path = "normal_mixed_before_change/edges.rs"]
mod edges;

#[path = "normal_mixed_before_change/indexed.rs"]
mod indexed;

#[path = "normal_mixed_before_change/nullable_index.rs"]
mod nullable_index;

#[path = "normal_mixed_before_change/nullable_keys.rs"]
mod nullable_keys;
