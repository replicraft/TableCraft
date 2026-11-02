#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};
use tablecraft_access::editing::{
    journal,
    offline::{Cause, OfflineWriteSession, Phase},
    sql,
};
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, RecoverableEditError,
    inspect_table_index_counter,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_desc_nullable_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(171);

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

fn counters(raw: &[u8], inserted: usize) {
    for (name, physical, value) in [
        ("PrimaryID", 0, 3 + inserted),
        ("PrimaryAlias", 0, 3 + inserted),
        ("GroupOrder", 1, 3),
        ("GroupAlias", 1, 3),
    ] {
        let count = inspect_table_index_counter(
            &mut Cursor::new(raw),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(count.physical_index(), physical);
        assert_eq!(count.value() as usize, value);
    }
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn typed_sql_nullable_desc_inserts_share_lease_and_keep_null_family_history() {
    let mut committed = 0;
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let mut session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();

            for (second, expected) in [(false, &case.single), (true, &case.batch)] {
                let before = if second { &case.single } else { &case.before };
                let staged = if typed {
                    let inserter = session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(inserter.name(), "Items");
                    assert_eq!(
                        inserter
                            .columns()
                            .iter()
                            .map(|column| column.name())
                            .collect::<Vec<_>>(),
                        ["Id", "Score", "Guard"]
                    );
                    assert!(!inserter.requires_field_list());
                    inserter
                        .plan_insert(sources::values(second), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    // Written columns deliberately differ from native order.
                    let sql = if second {
                        "INSERT INTO Items ([Guard], [Score], [Id]) VALUES (1, 18, 5)"
                    } else {
                        "INSERT INTO Items ([Guard], [Score], [Id]) VALUES (NULL, 17, 4)"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), *before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

                let (next, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(next.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), *expected);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let inserted = if second { 2 } else { 1 };
                counters(expected, inserted);

                // Refresh the same retained owner; every row and its handle
                // belongs to the new complete snapshot after this commit.
                let editor = next.edit_table("Items", &mut budget(), &|| false).unwrap();
                assert_eq!(
                    EditableTable::snapshot(&editor).rows(),
                    sources::rows(inserted, false)
                );
                session = editor.into_session();
                committed += 1;
            }
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
    assert_eq!(committed, 8);
}

#[test]
#[ignore = "requires all six source-first nullable/DESC INSERT views and SDK36 qualification"]
fn primary_refusals_after_null_insert_retain_last_receipt_file_and_lease() {
    let mut refused = 0;
    let mut seeds = 0;
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let seed = session
                .insert_table("Items", &mut budget(), &|| false)
                .unwrap()
                .plan_insert(sources::values(false), &mut budget(), &|| false)
                .unwrap();
            let (mut session, receipt) = seed.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(fs::read(&path).unwrap(), case.single);
            seeds += 1;
            for (id, token, wanted) in [
                (Value::Int32(4), "4", IndexConstraintViolation::DuplicateKey),
                (Value::Null, "NULL", IndexConstraintViolation::NullKey),
            ] {
                let failure = if typed {
                    let mut row = sources::values(false);
                    row[0] = id;
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(row, &mut budget(), &|| false)
                        .err()
                        .unwrap()
                } else {
                    let sql = format!(
                        "INSERT INTO Items ([Id], [Score], [Guard]) VALUES ({token}, 17, NULL)"
                    );
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .err()
                        .unwrap()
                };
                assert_eq!(failure.phase(), Phase::Planning);
                let error = match failure.cause() {
                    Cause::Editing(journal::Error::Plan(error)) => error,
                    Cause::InsertPlanning(sql::RecoverableInsertError::Plan(
                        tablecraft_query::InsertError::Backend(journal::Error::Plan(error)),
                    )) => error,
                    cause => panic!("expected native index constraint: {cause}"),
                };
                assert!(
                    matches!(error, RecoverableEditError::Edit(EditError::Inspect(
                    InspectError::IndexConstraint(actual)
                )) if *actual == wanted)
                );
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(failure.pair_state(), None);
                assert_eq!(fs::read(&path).unwrap(), case.single);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session = failure.into_session();
                refused += 1;
            }
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
    assert_eq!((seeds, refused), (4, 8));
}
