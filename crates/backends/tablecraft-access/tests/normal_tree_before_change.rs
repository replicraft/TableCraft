#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, io::Cursor, path::Path};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Failure, OfflineWriteSession, SessionState, StagedUpdate},
};
use tablecraft_ace::{
    IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints, read_table,
    read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_tree_before_change::{Case, RECIPES, Recipe, source};

#[derive(Clone, Copy, Debug)]
enum Transport {
    Typed,
    Sql,
}

const TRANSPORTS: [Transport; 2] = [Transport::Typed, Transport::Sql];
const SCOPE: CatalogScope = CatalogScope::new(100);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        // Frozen requests are on original Id2/3: user seeds9/-11, then
        // sequential Current+Old+1 yields final Score30/20.
        Recipe::NodeUniqueSwap => "UPDATE Items SET Score=49-20*Id WHERE Id>=2",
        Recipe::LeafPrefixLoss => "UPDATE Items SET Score=2147483636 WHERE Id=1",
        Recipe::NodePrefixLoss => "UPDATE Items SET Score=-51 WHERE Id=3",
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
                    assert_eq!(
                        editor.snapshot().resolve(handle).unwrap()[1],
                        Value::Int32(update.expected)
                    );
                    CellUpdate::new(handle, Value::Int32(update.replacement))
                })
                .collect::<Vec<_>>();
            editor.plan_updates("Score", &updates, work, &|| false)
        }
        Transport::Sql => {
            let query = parse_query(sql(recipe), ParseLimits::default()).unwrap();
            owner.plan_update(query.query(), work, None, &|| false)
        }
    }
}

#[test]
fn typed_and_sql_tree_sessions_match_eighteen_frozen_complete_images() {
    for header in [5, 6] {
        for recipe in RECIPES {
            let case = source(header, recipe);
            for transport in TRANSPORTS {
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
                assert_eq!(owner.current_outcome(), None);
                assert_eq!(fs::read(&database).unwrap(), case.expected);
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
                owner.release().unwrap();
                ExclusiveFile::open(&database).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            }
        }
    }
}

fn values(rows: &[[i32; 2]; 3]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| vec![Value::Int32(row[0]), Value::Int32(row[1])])
        .collect()
}

fn observe(
    database: &Path,
    rows: [[i32; 2]; 3],
    index_rows: [[i32; 2]; 3],
    work: &mut ResourceBudget,
) {
    locked(database);
    // This uncooperative readonly observation does not replace or unlock the
    // writer target. The session retains its original exclusive descriptor.
    let bytes = fs::read(database).unwrap();
    let actual = read_table(&mut Cursor::new(&bytes), SCOPE, "Items", work, &|| false).unwrap();
    assert_eq!(actual.rows(), values(&rows));
    let indexed = read_table_index(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        "EventKey",
        work,
        &|| false,
    )
    .unwrap();
    assert_eq!(indexed.rows(), values(&index_rows));
    let checked = check_table(&mut Cursor::new(&bytes), SCOPE, "Items", work, &|| false).unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
    let constraints =
        check_table_constraints(&mut Cursor::new(&bytes), SCOPE, "Items", work, &|| false).unwrap();
    assert_eq!(constraints.rows(), 3);
    assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
    assert!(constraints.relationships().is_empty());
    assert_eq!(constraints.fields().len(), 2);
    assert!(constraints.fields().iter().all(|field| {
        field.required() == RuleCheckStatus::NotDeclared
            && field.empty_text() == RuleCheckStatus::NotDeclared
            && field.validation() == RuleCheckStatus::NotDeclared
    }));
    locked(database);
}

#[test]
fn successive_node_edits_rebind_old_values_and_handles_under_the_same_lease() {
    for header in [5, 6] {
        let case = source(header, Recipe::NodeAscending);
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("target.accdb");
        fs::write(&database, &case.before).unwrap();
        let mut work = budget();
        let owner = OfflineWriteSession::open(&database, SCOPE, &mut work, &|| false).unwrap();
        let editor = owner.edit_table("Items", &mut work, &|| false).unwrap();
        let original_handle = editor.snapshot().row_handle(0).unwrap();
        assert_eq!(
            editor.snapshot().resolve(original_handle).unwrap()[1],
            Value::Int32(10)
        );
        let first = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(original_handle, Value::Int32(17))],
                &mut work,
                &|| false,
            )
            .unwrap();
        let (owner, first_receipt) = first
            .apply(&root.path().join("first"), &mut work, &|| false)
            .unwrap();
        assert_eq!(first_receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (first_receipt.matched_rows(), first_receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(fs::read(&database).unwrap(), case.expected);
        observe(
            &database,
            [[1, 28], [2, 20], [3, 30]],
            [[2, 20], [1, 28], [3, 30]],
            &mut work,
        );

        let editor = owner.edit_table("Items", &mut work, &|| false).unwrap();
        assert_eq!(
            editor.snapshot().resolve(original_handle),
            Err(SnapshotError::StaleRow)
        );
        let failure = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(original_handle, Value::Int32(35))],
                &mut work,
                &|| false,
            )
            .err()
            .expect("old handle must not select a fresh source row");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.last_completed(), Some(first_receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(fs::read(&database).unwrap(), case.expected);
        locked(&database);

        let editor = failure
            .into_session()
            .edit_table("Items", &mut work, &|| false)
            .unwrap();
        let fresh_handle = editor.snapshot().row_handle(0).unwrap();
        assert_eq!(
            editor.snapshot().resolve(fresh_handle).unwrap()[1],
            Value::Int32(28)
        );
        let second = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(fresh_handle, Value::Int32(35))],
                &mut work,
                &|| false,
            )
            .unwrap();
        assert_eq!((second.matched_rows(), second.changed_rows()), (1, 1));
        let (owner, second_receipt) = second
            .apply(&root.path().join("second"), &mut work, &|| false)
            .unwrap();
        assert_eq!(second_receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (second_receipt.matched_rows(), second_receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(owner.last_completed(), Some(second_receipt));
        assert_eq!(owner.current_outcome(), None);
        // Literal semantic/index expectations for the fresh Old28 frame:
        // Current35 + Old28 + 1 =64, crossing the original leaf partition.
        observe(
            &database,
            [[1, 64], [2, 20], [3, 30]],
            [[2, 20], [3, 30], [1, 64]],
            &mut work,
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(owner.state(), SessionState::Ready);
        owner.release().unwrap();
        ExclusiveFile::open(&database).unwrap().close().unwrap();
    }
}
