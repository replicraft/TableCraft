#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::FileExt,
};
use tablecraft_access::editing::offline::{Cause, OfflineWriteSession, Phase};
use tablecraft_ace::{NativeImage, NativeIntentError, RecoverableNativeDeletePlan, RowDelete};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::EditableTable;
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        DurableStorage, RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget, PairState},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_ordinary_delete::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(156);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn definition(case: &Case) -> usize {
    match case.data {
        110 => 107,
        127 => 125,
        _ => panic!("unexpected pinned Sentinel definition"),
    }
}

fn plan(target: &mut OfflineTarget, case: &Case) -> RecoverableNativeDeletePlan {
    let image = NativeImage::capture(target, SCOPE, &mut budget(), &|| false).unwrap();
    let base = case.data as usize * 4096;
    let plan = image
        .plan_recoverable_ordinary_row_deletes(
            "Sentinel",
            vec![RowDelete::new(
                0,
                case.before[base + 4030..base + 4096].to_vec(),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), case.before);
    assert_eq!(plan.after_bytes(), case.after);
    plan
}

#[test]
#[ignore = "requires all four pinned ordinary DELETE SOURCE images"]
fn interrupted_delete_commit_recovers_pending_on_the_same_retained_session() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let observer = File::open(&path).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let query =
            parse_query("DELETE FROM Sentinel WHERE [ID]=23", ParseLimits::default()).unwrap();
        let staged = session
            .plan_delete(query.query(), &mut budget(), None, &|| false)
            .unwrap();
        let saw_deleted_count = Cell::new(false);
        let observer_error = RefCell::new(None);
        let count_at = (definition(&case) * 4096 + 16) as u64;
        let cancelled = || {
            let mut count = [0; 4];
            if let Err(error) = observer.read_exact_at(&mut count, count_at) {
                *observer_error.borrow_mut() = Some(error);
                return true;
            }
            if count == [0; 4] {
                saw_deleted_count.set(true);
                return true;
            }
            false
        };
        let failure = staged
            .apply(&bundle, &mut budget(), &cancelled)
            .err()
            .unwrap();
        assert!(observer_error.borrow().is_none());
        assert!(saw_deleted_count.get());
        assert_eq!(failure.phase(), Phase::Commit);
        assert!(matches!(failure.cause(), Cause::Commit(_)));
        assert!(!failure.invalid_input());
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.pair_state(), Some(PairState::RecoveryRequired));
        let current = fs::read(&path).unwrap();
        assert_eq!(current.len(), case.before.len());
        let count_start = count_at as usize;
        assert_eq!(&current[count_start..count_start + 4], &[0; 4]);
        // TD and DATA share a64KiB write chunk in these originals. The observed
        // state may be whole AFTER; assert admitted origins without claiming a
        // naturally observed mixed page boundary that the transport lacks.
        for (at, byte) in current.iter().enumerate() {
            assert!(case.before.get(at) == Some(byte) || case.after.get(at) == Some(byte));
        }
        assert_eq!(&fs::read(bundle.join("intent")).unwrap()[..8], b"TCND0001");
        assert_eq!(&fs::read(bundle.join("journal")).unwrap()[..8], b"TCIMG001");
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = failure
            .into_session()
            .recover_pending(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::RolledBack));
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(EditableTable::snapshot(&editor).rows().len(), 1);
        editor.into_session().release().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
#[ignore = "requires all four pinned ordinary DELETE SOURCE images"]
fn authored_td_only_partial_state_restarts_through_actual_delete_session_recovery() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
        let plan = plan(&mut target, &case);
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let mut pair =
            OfflineJournalPair::create(target, &bundle, wire, &mut budget(), &|| false).unwrap();
        {
            let (mut target, mut journal) = pair.storage().unwrap();
            drop(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap(),
            );
        }
        let at = definition(&case) * 4096 + 16;
        let mut partial = case.before.clone();
        partial[at..at + 4].copy_from_slice(&case.after[at..at + 4]);
        {
            // Explicit authored interruption fixture, not an observed OS
            // partial write. Journal images remain exactly the frozen sources.
            let (mut target, _) = pair.storage().unwrap();
            target.seek(SeekFrom::Start(at as u64)).unwrap();
            target.write_all(&case.after[at..at + 4]).unwrap();
            target.sync_storage().unwrap();
        }
        assert_eq!(fs::read(&path).unwrap(), partial);
        let root_word = case.data as usize * 4096 + 14;
        assert_eq!(&partial[root_word..root_word + 2], &[0xbe, 0x0f]);
        pair.release().unwrap();
        drop(pair);
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let (session, receipt) = session
            .recover_update(&bundle, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::RolledBack));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        session.release().unwrap();
    }
}

#[test]
#[ignore = "requires all four pinned ordinary DELETE SOURCE images"]
fn malformed_action_family_refuses_without_target_or_retained_bundle_changes() {
    for case in sources::load_all(verify) {
        for update_family in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
            let plan = plan(&mut target, &case);
            let mut wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            if update_family {
                // TCNI7 framing with DELETE's profile1 must fail its exact
                // chosen UPDATE decoder, with no retry as TCND.
                wire[..8].copy_from_slice(b"TCNI0007");
            } else {
                wire[7] = b'2';
            }
            let mut pair =
                OfflineJournalPair::create(target, &bundle, wire, &mut budget(), &|| false)
                    .unwrap();
            {
                let (mut target, mut journal) = pair.storage().unwrap();
                drop(
                    plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap(),
                );
            }
            let retained =
                ["binding", "intent", "journal"].map(|name| fs::read(bundle.join(name)).unwrap());
            pair.release().unwrap();
            drop(pair);
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let failure = session
                .recover_update(&bundle, &mut budget(), &|| false)
                .err()
                .unwrap();
            let expected = if update_family {
                NativeIntentError::UnsupportedProfile
            } else {
                NativeIntentError::UnsupportedVersion
            };
            assert!(matches!(failure.cause(), Cause::Intent(error) if error == &expected));
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            for (name, bytes) in ["binding", "intent", "journal"].into_iter().zip(retained) {
                assert_eq!(fs::read(bundle.join(name)).unwrap(), bytes);
            }
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
        }
    }
}
