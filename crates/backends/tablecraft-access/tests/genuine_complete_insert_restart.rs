#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Seek, SeekFrom, Write},
    path::PathBuf,
};
use tablecraft_access::editing::{
    journal::{NativeJournalInsertPlan, NativeJournalInserter},
    offline::{CompletionKind, OfflineWriteSession, SessionState, VerifiedImage},
};
use tablecraft_ace::{NativeImage, inspect_table_index_counter};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertableTable};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        DurableStorage, RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
        prepare_expected, prepare_resizable_expected,
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    genuine_eof_insert as eof, genuine_indexed_insert as indexed,
};

const SCOPE: CatalogScope = CatalogScope::new(196);

struct Case {
    header: u8,
    recipe: &'static str,
    table: &'static str,
    before: Vec<u8>,
    after: Vec<u8>,
    values: Vec<Value>,
    rows: Vec<Vec<Value>>,
    count_at: usize,
    indexed_inserted: usize,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 2_000_000, 2_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn cases() -> Vec<Case> {
    let mut result = Vec::new();
    for case in indexed::load_all(verify) {
        result.push(Case {
            header: case.header,
            recipe: "indexed-first",
            table: "Items",
            before: case.before,
            after: case.single.clone(),
            values: indexed::values(false),
            rows: indexed::rows(1),
            count_at: case.definition as usize * 4096 + 16,
            indexed_inserted: 1,
        });
        result.push(Case {
            header: case.header,
            recipe: "indexed-second",
            table: "Items",
            before: case.single,
            after: case.batch,
            values: indexed::values(true),
            rows: indexed::rows(2),
            count_at: case.definition as usize * 4096 + 16,
            indexed_inserted: 2,
        });
    }
    for case in eof::load_all(verify).into_iter().filter(|case| !case.batch) {
        let rows = eof::rows(&case, true);
        result.push(Case {
            header: if case.definition == 107 { 5 } else { 6 },
            recipe: "eof-single",
            table: "Sentinel",
            before: case.before,
            after: case.after,
            values: eof::values(260),
            rows,
            count_at: case.definition * 4096 + 16,
            indexed_inserted: 0,
        });
    }
    assert_eq!(result.len(), 6);
    result
}

fn wire(case: &Case) -> Vec<u8> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TCNA4_HOST_WIRES_DIR").unwrap());
    assert!(directory.is_absolute());
    let bytes =
        fs::read(directory.join(format!("h{}-{}.intent", case.header, case.recipe))).unwrap();
    assert_eq!(&bytes[..8], b"TCNA0004");
    assert_eq!(
        bytes.len(),
        if case.indexed_inserted == 0 { 112 } else { 117 }
    );
    bytes
}

fn counters(bytes: &[u8], inserted: usize) {
    for (name, physical, historical) in [
        ("PrimaryID", 0, 3 + inserted),
        ("PrimaryAlias", 0, 3 + inserted),
        ("GroupOrder", 1, 4),
        ("GroupAlias", 1, 4),
    ] {
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.physical_index(), physical);
        assert_eq!(counter.value() as usize, historical);
    }
}

#[test]
#[ignore = "requires six indexed and eight EOF source views plus independently frozen TCNA4 wires"]
fn complete_host_upgrade_preserves_both_index_trees_aliases_history_and_existing_eof_images() {
    for case in cases() {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let inserter =
            NativeJournalInserter::new(image, case.table, &mut budget(), &|| false).unwrap();
        let plan = inserter
            .plan_insert(case.values.clone(), &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(&plan, NativeJournalInsertPlan::CompleteTyped(_)));
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(
            plan.encode_intent(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        if case.indexed_inserted != 0 {
            counters(plan.after_bytes(), case.indexed_inserted);
        }
    }
}

#[test]
#[ignore = "requires qualified indexed/EOF source directories and frozen V4 host wires"]
fn tcna4_fixed_and_growing_cold_restart_keeps_mixed_pending_and_retired_confirmation_distinct() {
    let mut contexts = 0;
    for case in cases() {
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
                // Generic IO prepares the independently qualified images and
                // literal original intent; no native producer creates fixtures.
                let mut pair = OfflineJournalPair::create(
                    target,
                    &bundle,
                    wire(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let growing = case.after.len() > case.before.len();
                if growing {
                    let (mut target, mut journal) = pair.resizable_storage().unwrap();
                    let prepared = prepare_resizable_expected(
                        &mut target,
                        &mut journal,
                        &case.before,
                        &case.after,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        prepared.rollback(&mut budget(), &|| false).unwrap();
                    } else {
                        drop(prepared);
                    }
                } else {
                    let (mut target, mut journal) = pair.storage().unwrap();
                    let prepared = prepare_expected(
                        &mut target,
                        &mut journal,
                        &case.before,
                        &case.after,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        prepared.rollback(&mut budget(), &|| false).unwrap();
                    } else {
                        drop(prepared);
                    }
                }
                if !committed && !retired {
                    let (mut target, _) = pair.resizable_storage().unwrap();
                    // A true mixed target: changed TD count, original DATA and
                    // indexes. Growing cases also contain only 17 EOF bytes.
                    target.seek(SeekFrom::Start(case.count_at as u64)).unwrap();
                    target
                        .write_all(&case.after[case.count_at..case.count_at + 4])
                        .unwrap();
                    if growing {
                        target
                            .seek(SeekFrom::Start(case.before.len() as u64))
                            .unwrap();
                        target
                            .write_all(&case.after[case.before.len()..case.before.len() + 17])
                            .unwrap();
                    }
                    target.sync_storage().unwrap();
                    let partial = fs::read(&path).unwrap();
                    assert_ne!(partial, case.before);
                    assert_ne!(partial, case.after);
                    assert_eq!(
                        partial.len(),
                        case.before.len() + if growing { 17 } else { 0 }
                    );
                }
                let selected = if retired {
                    pair.retire(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                    .unwrap();
                    let candidates = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|entry| entry.is_dir())
                        .collect::<Vec<_>>();
                    assert_eq!(candidates.len(), 1);
                    candidates.into_iter().next().unwrap()
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
                    assert_eq!(receipt.kind(), CompletionKind::Confirmed);
                    assert_eq!(receipt.outcome(), None);
                    assert_eq!(
                        receipt.verified_image(),
                        Some(if committed {
                            VerifiedImage::Replacement
                        } else {
                            VerifiedImage::Original
                        })
                    );
                } else {
                    assert_eq!(receipt.kind(), CompletionKind::Recovered);
                    assert_eq!(
                        receipt.outcome(),
                        Some(if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        })
                    );
                }
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), None);
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(session.state(), SessionState::Ready);
                let expected = if committed { &case.after } else { &case.before };
                assert_eq!(fs::read(&path).unwrap(), *expected);
                assert!(!selected.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                if committed {
                    let editor = session
                        .edit_table(case.table, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(EditableTable::snapshot(&editor).rows(), case.rows);
                    editor.into_session().release().unwrap();
                } else {
                    session.release().unwrap();
                }
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 24);
}
