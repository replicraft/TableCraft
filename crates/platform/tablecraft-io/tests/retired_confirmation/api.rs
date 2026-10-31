use super::{budget, retired_source};
use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{
    IoError,
    journal::filesystem::{OfflineJournalPair, OfflineTarget, PairState},
    journal::{DurableStorage, JournalError, RecoveryOutcome, inspect_v1},
};

struct Observe {
    data: Cursor<Vec<u8>>,
    writes: usize,
    syncs: usize,
    fail_sync: bool,
}

impl Observe {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            data: Cursor::new(bytes),
            writes: 0,
            syncs: 0,
            fail_sync: false,
        }
    }
}

impl Read for Observe {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.data.read(bytes)
    }
}
impl Seek for Observe {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.data.seek(position)
    }
}
impl Write for Observe {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        panic!("confirmation attempted a write")
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("confirmation attempted a flush")
    }
}
impl DurableStorage for Observe {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.data.get_ref().len() as u64)
    }
    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.syncs += 1;
        if self.fail_sync {
            return Err(IoError::Io(io::ErrorKind::Other));
        }
        Ok(())
    }
}

#[test]
fn completed_confirmation_requires_exact_full_image_and_never_writes() {
    for committed in [false, true] {
        let source = retired_source(committed);
        let bytes = fs::read(source.retired.join("journal")).unwrap();
        for target in [
            b"original".as_slice(),
            b"modified".as_slice(),
            b"origfied".as_slice(),
            b"unrelated".as_slice(),
        ] {
            let mut journal = Observe::new(bytes.clone());
            let mut target = Observe::new(target.to_vec());
            let original = target.data.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let result = checked.confirm_completed(&mut target, &mut budget(), &|| false);
            let expected = if committed { b"modified" } else { b"original" };
            if original == expected {
                assert_eq!(
                    result.unwrap(),
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!((target.syncs, journal.syncs), (1, 1));
            } else {
                assert_eq!(result, Err(JournalError::TargetMismatch));
                assert_eq!((target.syncs, journal.syncs), (0, 0));
            }
            assert_eq!(target.data.get_ref(), &original);
            assert_eq!(journal.data.get_ref(), &bytes);
            assert_eq!((target.writes, journal.writes), (0, 0));
        }
    }
}

#[test]
fn retired_pair_reopens_readonly_confirms_then_cleans_without_rename() {
    for committed in [false, true] {
        let source = retired_source(committed);
        let before = fs::read(&source.target).unwrap();
        let journal_before = fs::read(source.retired.join("journal")).unwrap();
        let owner = OfflineTarget::open(&source.target, &mut budget(), &|| false).unwrap();
        let mut pair =
            OfflineJournalPair::open_retired(owner, &source.retired, 1024, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(pair.state(), PairState::RetiredVerificationRequired);
        assert!(pair.storage().is_err());
        let outcome = {
            let (mut target, mut journal) = pair.confirmation_storage().unwrap();
            assert!(target.write(b"x").is_err());
            assert!(target.flush().is_err());
            assert!(journal.write(b"x").is_err());
            assert!(journal.flush().is_err());
            inspect_v1(&mut journal, &mut budget(), &|| false)
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap()
        };
        assert_eq!(fs::read(&source.target).unwrap(), before);
        assert_eq!(
            fs::read(source.retired.join("journal")).unwrap(),
            journal_before
        );
        pair.acknowledge_retired(outcome).unwrap();
        assert_eq!(pair.state(), PairState::Retired);
        pair.cleanup_retired().unwrap();
        pair.release().unwrap();
        assert!(!source.retired.exists());
        assert_eq!(fs::read(source.target).unwrap(), before);
        assert_eq!(fs::read_dir(source.root.path()).unwrap().count(), 1);
    }
}

#[test]
fn retired_pair_requires_exact_reserved_leaf_and_intact_original_roles() {
    let source = retired_source(true);
    let before = fs::read(&source.target).unwrap();
    let owner = OfflineTarget::open(&source.target, &mut budget(), &|| false).unwrap();
    let failed =
        OfflineJournalPair::open_existing(owner, &source.retired, 1024, &mut budget(), &|| false)
            .err()
            .unwrap();
    let mut owner = failed.into_target();
    owner.release().unwrap();
    drop(owner);
    let wrong = source.root.path().join("wrong-retired-name");
    fs::rename(&source.retired, &wrong).unwrap();
    let owner = OfflineTarget::open(&source.target, &mut budget(), &|| false).unwrap();
    let failed = OfflineJournalPair::open_retired(owner, &wrong, 1024, &mut budget(), &|| false)
        .err()
        .unwrap();
    let mut owner = failed.into_target();
    owner.release().unwrap();
    drop(owner);
    fs::rename(&wrong, &source.retired).unwrap();
    fs::remove_file(source.retired.join("journal")).unwrap();
    let owner = OfflineTarget::open(&source.target, &mut budget(), &|| false).unwrap();
    assert!(
        OfflineJournalPair::open_retired(owner, &source.retired, 1024, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(fs::read(source.target).unwrap(), before);
}

#[test]
fn completed_confirmation_limits_and_sync_failures_do_not_repair_images() {
    let source = retired_source(false);
    let bytes = fs::read(source.retired.join("journal")).unwrap();
    let mut journal = Observe::new(bytes.clone());
    let mut target = Observe::new(b"original".to_vec());
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let mut observed = budget();
    checked
        .confirm_completed(&mut target, &mut observed, &|| false)
        .unwrap();
    for resource in [Resource::Bytes, Resource::Steps] {
        let used = budget().remaining(resource) - observed.remaining(resource);
        for limit in [used - 1, used] {
            let mut journal = Observe::new(bytes.clone());
            let mut target = Observe::new(b"original".to_vec());
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let mut allowance = ResourceBudget::new(
                if resource == Resource::Bytes {
                    limit
                } else {
                    10_000_000
                },
                0,
                if resource == Resource::Steps {
                    limit
                } else {
                    10_000_000
                },
            );
            assert_eq!(
                checked
                    .confirm_completed(&mut target, &mut allowance, &|| false)
                    .is_ok(),
                limit == used
            );
            assert_eq!(target.data.get_ref(), b"original");
            assert_eq!((target.writes, journal.writes), (0, 0));
        }
    }
    for journal_fault in [false, true] {
        let mut journal = Observe::new(bytes.clone());
        let mut target = Observe::new(b"original".to_vec());
        if journal_fault {
            journal.fail_sync = true;
        } else {
            target.fail_sync = true;
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            checked
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(target.data.get_ref(), b"original");
        assert_eq!((target.writes, journal.writes), (0, 0));
    }
}

#[test]
fn partial_marker_and_final_observed_cancellation_never_repair_target() {
    let source = retired_source(false);
    let mut bytes = fs::read(source.retired.join("journal")).unwrap();
    bytes.extend_from_slice(b"COM");
    for image in [b"original", b"modified", b"origfied"] {
        let mut journal = Observe::new(bytes.clone());
        let mut target = Observe::new(image.to_vec());
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let result = checked.confirm_completed(&mut target, &mut budget(), &|| false);
        assert_eq!(
            result,
            if image == b"original" {
                Ok(RecoveryOutcome::RolledBack)
            } else {
                Err(JournalError::TargetMismatch)
            }
        );
        assert_eq!(target.data.get_ref(), image);
        assert_eq!((target.writes, journal.writes), (0, 0));
    }

    let mut journal = Observe::new(bytes.clone());
    let mut target = Observe::new(b"original".to_vec());
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let calls = Cell::new(0);
    checked
        .confirm_completed(&mut target, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let last = calls.get();
    let mut journal = Observe::new(bytes);
    let mut target = Observe::new(b"original".to_vec());
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    calls.set(0);
    assert!(
        checked
            .confirm_completed(&mut target, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == last
            })
            .is_err()
    );
    assert_eq!(calls.get(), last);
    assert_eq!((target.syncs, journal.syncs), (1, 1));
    assert_eq!((target.writes, journal.writes), (0, 0));
    assert_eq!(target.data.get_ref(), b"original");
}

#[test]
fn retired_entry_replacement_blocks_barrier_retry_and_preserves_current_image_fact() {
    let source = retired_source(true);
    let owner = OfflineTarget::open(&source.target, &mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::open_retired(owner, &source.retired, 1024, &mut budget(), &|| false)
            .unwrap();
    let outcome = {
        let (mut target, mut journal) = pair.confirmation_storage().unwrap();
        inspect_v1(&mut journal, &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap()
    };
    let moved = source.root.path().join("moved-intact");
    fs::rename(&source.retired, &moved).unwrap();
    fs::create_dir(&source.retired).unwrap();
    let failed = pair.acknowledge_retired(outcome).unwrap_err();
    assert_eq!(failed.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(pair.known_outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(pair.state(), PairState::RetirementBlocked);
    assert!(pair.confirm_retirement(&mut budget(), &|| false).is_err());
    assert!(pair.cleanup_retired().is_err());
    pair.release().unwrap();
    assert_eq!(fs::read(&source.target).unwrap(), b"modified");
    assert!(moved.join("journal").exists());
    assert_eq!(fs::read_dir(&source.retired).unwrap().count(), 0);
}
