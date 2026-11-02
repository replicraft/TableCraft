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
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::InsertableTable;
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_inline_lval_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(195);

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

fn sql(recipe: &str) -> &'static str {
    match recipe {
        "null" => {
            "INSERT INTO InlineValues ([BlobValue], [Id], [MemoValue]) VALUES (NULL, 2, NULL)"
        }
        "memo-empty-ole-null" => {
            "INSERT INTO InlineValues ([MemoValue], [BlobValue], [Id]) VALUES ('', NULL, 2)"
        }
        _ => panic!("no invented SQL binary literal or NUL syntax"),
    }
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL source views"]
fn inline_lval_typed_and_sql_saves_use_real_sessions_and_preserve_exact_source_images() {
    let mut saves = [0; 2];
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            if !typed && !matches!(case.recipe, "null" | "memo-empty-ole-null") {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let bundle = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                session
                    .insert_table("InlineValues", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(sources::values(case.recipe), &mut budget(), &|| false)
                    .unwrap()
            } else {
                let query = parse_query(sql(case.recipe), ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let rows = read_table(
                &mut Cursor::new(&case.after),
                SCOPE,
                "InlineValues",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), sources::rows(case.recipe));
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [16, 4]);
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL source views"]
fn inline_oversize_typed_and_sql_requests_preserve_file_lease_and_no_bundle() {
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.recipe == "ascii")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let mut session =
            OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        for kind in 0..4 {
            let failure = if kind < 3 {
                let row = match kind {
                    0 => vec![Value::Int32(2), Value::Text("a".repeat(63)), Value::Null],
                    1 => vec![Value::Int32(2), Value::Text("β".repeat(33)), Value::Null],
                    _ => vec![Value::Int32(2), Value::Null, Value::Binary(vec![0; 65])],
                };
                session
                    .insert_table("InlineValues", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(row, &mut budget(), &|| false)
                    .err()
                    .unwrap()
            } else {
                let statement = format!(
                    "INSERT INTO InlineValues ([Id], [MemoValue], [BlobValue]) VALUES (2, '{}', NULL)",
                    "a".repeat(63)
                );
                let query = parse_query(&statement, ParseLimits::default()).unwrap();
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
                cause => panic!("expected inline-only native refusal: {cause}"),
            };
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
            ));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            session = failure.into_session();
        }
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
    }
}
