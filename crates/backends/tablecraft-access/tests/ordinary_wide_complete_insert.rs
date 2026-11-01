#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, io::Cursor};
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_ace::read_table;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::InsertableTable;
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_ordinary_wide_insert::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(216);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn typed_all_exact_scalar_rows_and_honest_sql_null_rows_save_literal_complete_images() {
    let mut saves = [0; 2];
    for case in sources::CASES {
        for header in [5_u32, 6] {
            let mut source = sources::source(case);
            source.before[20..24].copy_from_slice(&header.to_le_bytes());
            source.after[20..24].copy_from_slice(&header.to_le_bytes());
            for typed in [true, false] {
                let sql = match case {
                    Case::FixedNull => Some(
                        "INSERT INTO Items VALUES (False, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
                    ),
                    Case::DecimalNull => {
                        Some("INSERT INTO Items VALUES (101, NULL, NULL, NULL, False)")
                    }
                    _ => None,
                };
                if !typed && sql.is_none() {
                    continue;
                }
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &source.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let inserter = session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(
                        inserter
                            .columns()
                            .iter()
                            .map(|column| column.name())
                            .collect::<Vec<_>>(),
                        source.columns
                    );
                    assert!(!inserter.requires_field_list());
                    inserter
                        .plan_insert(source.values.clone(), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(sql.unwrap(), ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), source.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), source.after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let table = read_table(
                    &mut Cursor::new(fs::read(&path).unwrap()),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let mut expected = source.rows_before.clone();
                expected.push(source.values.clone());
                assert_eq!(table.rows(), expected);
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves[usize::from(!typed)] += 1;
            }
        }
    }
    assert_eq!(saves, [16, 4]);
}
