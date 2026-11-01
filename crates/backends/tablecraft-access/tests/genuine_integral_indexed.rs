#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState, VerifiedImage};
use tablecraft_ace::{NativeImage, RowDelete, RowInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_integral_indexed::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(198);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let hash = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, expected);
}

fn sql(case: &Case) -> String {
    if case.operation == "insert" {
        assert_eq!(case.recipe, "null-family");
        return format!(
            "INSERT INTO {} ([Marker], [KeyValue], [Id]) VALUES (68, NULL, 4)",
            case.table
        );
    }
    let filter = match case.recipe {
        "minimum" => " WHERE [Id]=1",
        "null" => " WHERE [Id]=2",
        "exterior" => " WHERE [Id]=1 OR [Id]=3",
        "all" => "",
        _ => panic!("outside literal deletion recipes"),
    };
    format!("DELETE FROM {}{filter}", case.table)
}

fn unchanged(path: &Path, bundle: &Path, before: &[u8]) {
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!bundle.exists());
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
#[ignore = "requires 34 native/SDK-qualified numeric source images"]
fn numeric_typed_sql_actions_preserve_literal_images_carriers_receipts_and_lease() {
    let mut saves = [0; 2];
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            if case.operation == "insert"
                && (case.recipe == "batch-families" || (!typed && case.recipe != "null-family"))
            {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let bundle = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let changed;
            let (session, receipt) = if case.operation == "insert" {
                let staged = if typed {
                    session
                        .insert_table(case.table, &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(
                            sources::insert_values(case.table, case.recipe).remove(0),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let query = parse_query(&sql(&case), ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                changed = 1;
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            } else {
                let selected = sources::ordinals(case.recipe, false);
                let staged = if typed {
                    let editor = session
                        .edit_table(case.table, &mut budget(), &|| false)
                        .unwrap();
                    let handles = selected
                        .iter()
                        .map(|slot| EditableTable::snapshot(&editor).row_handle(*slot).unwrap())
                        .collect::<Vec<_>>();
                    DeletableTable::plan_deletes(editor, &handles, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(&sql(&case), ParseLimits::default()).unwrap();
                    session
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (staged.matched_rows(), staged.deleted_rows()),
                    (selected.len(), selected.len())
                );
                changed = selected.len();
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(changed));
            assert_eq!(
                receipt.matched_rows(),
                if case.operation == "insert" {
                    None
                } else {
                    Some(changed)
                }
            );
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                EditableTable::snapshot(&editor).rows(),
                sources::rows(&case)
            );
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [28, 20]);
}

fn pair(
    case: &Case,
    path: &Path,
    bundle: &Path,
    committed: bool,
    retired: bool,
) -> OfflineJournalPair {
    let mut target = OfflineTarget::open(path, &mut budget(), &|| false).unwrap();
    let image = NativeImage::capture(&mut target, SCOPE, &mut budget(), &|| false).unwrap();
    if case.operation == "insert" {
        let rows = sources::insert_values(case.table, case.recipe)
            .into_iter()
            .map(RowInsert::new)
            .collect();
        let plan = image
            .plan_recoverable_ordinary_row_inserts(case.table, rows, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&intent[..8], b"TCNA0001");
        let mut pair =
            OfflineJournalPair::create(target, bundle, intent, &mut budget(), &|| false).unwrap();
        {
            let (mut target, mut journal) = pair.storage().unwrap();
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
            } else if retired {
                assert_eq!(
                    prepared.rollback(&mut budget(), &|| false).unwrap(),
                    RecoveryOutcome::RolledBack
                );
            } else {
                drop(prepared);
            }
        }
        pair
    } else {
        let rows = sources::ordinals(case.recipe, false)
            .into_iter()
            .map(|slot| RowDelete::new(slot, sources::record(slot)))
            .collect();
        let plan = image
            .plan_recoverable_ordinary_row_deletes(case.table, rows, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&intent[..8], b"TCND0001");
        let mut pair =
            OfflineJournalPair::create(target, bundle, intent, &mut budget(), &|| false).unwrap();
        {
            let (mut target, mut journal) = pair.storage().unwrap();
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 2);
            } else if retired {
                assert_eq!(
                    prepared.rollback(&mut budget(), &|| false).unwrap(),
                    RecoveryOutcome::RolledBack
                );
            } else {
                drop(prepared);
            }
        }
        pair
    }
}

#[test]
#[ignore = "requires 34 native/SDK-qualified numeric source images"]
fn numeric_original_tcna_tcnd_replay_uses_actual_active_and_retired_session_dispatch() {
    let mut contexts = 0;
    for case in sources::load_all(verify).into_iter().filter(|case| {
        (case.operation == "insert" && case.recipe == "null-family")
            || (case.operation == "delete" && case.recipe == "exterior")
    }) {
        for committed in [false, true] {
            for retired in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("target.accdb");
                let bundle = dir.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let mut pair = pair(&case, &path, &bundle, committed, retired);
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let selected = if retired {
                    pair.retire(outcome).unwrap();
                    let mut dirs = fs::read_dir(dir.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| path.is_dir())
                        .collect::<Vec<_>>();
                    assert_eq!(dirs.len(), 1);
                    dirs.remove(0)
                } else {
                    bundle.clone()
                };
                pair.release().unwrap();
                drop(pair);
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let (session, receipt) = if retired {
                    session
                        .confirm_retired(&selected, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    session
                        .recover_update(&selected, &mut budget(), &|| false)
                        .unwrap()
                };
                if retired {
                    assert_eq!(
                        receipt.verified_image(),
                        Some(if committed {
                            VerifiedImage::Replacement
                        } else {
                            VerifiedImage::Original
                        })
                    );
                } else {
                    assert_eq!(receipt.outcome(), Some(outcome));
                }
                assert_eq!(
                    fs::read(&path).unwrap().as_slice(),
                    if committed {
                        case.after.as_slice()
                    } else {
                        case.before.as_slice()
                    }
                );
                assert_eq!(session.state(), SessionState::Ready);
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 32);
}
