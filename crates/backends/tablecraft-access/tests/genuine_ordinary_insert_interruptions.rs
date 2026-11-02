#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    fs::{self, File},
    os::unix::fs::FileExt,
};
use tablecraft_access::editing::offline::{Cause, OfflineWriteSession, Phase};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::EditableTable;
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{RecoveryOutcome, filesystem::PairState},
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(161);

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

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn interrupted_insert_commit_recovers_pending_on_the_same_retained_session() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let observer = File::open(&path).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let query = parse_query(
            "INSERT INTO Sentinel ([ID], Marker, Bytes) VALUES (24, 'new', NULL)",
            ParseLimits::default(),
        )
        .unwrap();
        let staged = session
            .plan_insert(query.query(), &mut budget(), &|| false)
            .unwrap();
        let saw_inserted_count = Cell::new(false);
        let observer_error = RefCell::new(None);
        let count_at = (case.definition as usize * 4096 + 16) as u64;
        let cancelled = || {
            let mut count = [0; 4];
            if let Err(error) = observer.read_exact_at(&mut count, count_at) {
                *observer_error.borrow_mut() = Some(error);
                return true;
            }
            if count == [2, 0, 0, 0] {
                saw_inserted_count.set(true);
                return true;
            }
            false
        };
        let failure = staged
            .apply(&bundle, &mut budget(), &cancelled)
            .err()
            .unwrap();
        assert!(observer_error.borrow().is_none());
        assert!(saw_inserted_count.get());
        assert_eq!(failure.phase(), Phase::Commit);
        assert!(matches!(failure.cause(), Cause::Commit(_)));
        assert!(!failure.invalid_input());
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.pair_state(), Some(PairState::RecoveryRequired));
        let current = fs::read(&path).unwrap();
        assert_eq!(current.len(), case.before.len());
        let count_start = count_at as usize;
        assert_eq!(&current[count_start..count_start + 4], &[2, 0, 0, 0]);
        // TD and DATA share a 64 KiB write chunk in these originals. The observed
        // state may be whole AFTER; assert admitted origins without claiming a
        // naturally observed mixed page boundary that the transport lacks.
        for (at, byte) in current.iter().enumerate() {
            assert!(case.before.get(at) == Some(byte) || case.after.get(at) == Some(byte));
        }
        assert_eq!(&fs::read(bundle.join("intent")).unwrap()[..8], b"TCNA0004");
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
